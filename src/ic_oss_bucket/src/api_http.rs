use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use candid::{define_function, CandidType};
use hyperx::header::{Charset, ContentDisposition, DispositionParam, DispositionType};
use hyperx::header::{ContentRangeSpec, Header, IfRange, Range, Raw};
use ic_http_certification::{HeaderField, HttpRequest};
use ic_oss_types::{
    file::{UrlFileParam, CHUNK_SIZE, MAX_FILE_SIZE_PER_CALL},
    to_cbor_bytes,
};
use ic_stable_structures::Storable;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use std::path::Path;
use std::str::FromStr;

use crate::{permission, store, SECONDS};

#[derive(CandidType, Deserialize, Clone, Default)]
pub struct HttpStreamingResponse {
    pub status_code: u16,
    pub headers: Vec<HeaderField>,
    pub body: ByteBuf,
    pub upgrade: Option<bool>,
    pub streaming_strategy: Option<StreamingStrategy>,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct StreamingCallbackToken {
    pub id: u32,
    pub chunk_index: u32,
    pub chunks: u32,
    pub token: Option<ByteBuf>,
}

impl StreamingCallbackToken {
    pub fn next(self) -> Option<StreamingCallbackToken> {
        if self.chunk_index + 1 >= self.chunks {
            None
        } else {
            Some(StreamingCallbackToken {
                id: self.id,
                chunk_index: self.chunk_index + 1,
                chunks: self.chunks,
                token: self.token,
            })
        }
    }
}

define_function!(pub CallbackFunc : (StreamingCallbackToken) -> (StreamingCallbackHttpResponse) query);

#[derive(CandidType, Deserialize, Clone)]
pub enum StreamingStrategy {
    Callback {
        token: StreamingCallbackToken,
        callback: CallbackFunc,
    },
}

#[derive(CandidType, Deserialize, Clone)]
pub struct StreamingCallbackHttpResponse {
    pub body: ByteBuf,
    pub token: Option<StreamingCallbackToken>,
}

static STREAMING_CALLBACK: Lazy<CallbackFunc> =
    Lazy::new(|| CallbackFunc::new(ic_cdk::id(), "http_request_streaming_callback".to_string()));

fn create_strategy(arg: StreamingCallbackToken) -> Option<StreamingStrategy> {
    arg.next().map(|token| StreamingStrategy::Callback {
        token,
        callback: STREAMING_CALLBACK.clone(),
    })
}

static OCTET_STREAM: &str = "application/octet-stream";
static IC_CERTIFICATE_HEADER: &str = "ic-certificate";
static IC_CERTIFICATE_EXPRESSION_HEADER: &str = "ic-certificateexpression";

// request url example:
// https://mmrxu-fqaaa-aaaap-ahhna-cai.icp0.io/f/1
// http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/f/1 // download file by id 1
// http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/h/8546ffa4296a6960e9e64e95de178d40c231a0cd358a65477bc56a105dda1c1d //download file by hash 854...
#[ic_cdk::query(hidden = true)]
fn http_request(request: HttpRequest) -> HttpStreamingResponse {
    let witness = store::state::http_tree_with(|t| {
        t.witness(&store::state::DEFAULT_CERT_ENTRY, request.url())
            .expect("get witness failed")
    });
    let certified_data = ic_cdk::api::data_certificate().expect("no data certificate available");
    let mut headers = vec![
        ("content-type".to_string(), "text/plain".to_string()),
        ("x-content-type-options".to_string(), "nosniff".to_string()),
        (
            IC_CERTIFICATE_EXPRESSION_HEADER.to_string(),
            store::state::DEFAULT_CEL_EXPR.clone(),
        ),
        (
            IC_CERTIFICATE_HEADER.to_string(),
            format!(
                "certificate=:{}:, tree=:{}:, expr_path=:{}:, version=2",
                BASE64.encode(certified_data),
                BASE64.encode(to_cbor_bytes(&witness)),
                BASE64.encode(to_cbor_bytes(
                    &store::state::DEFAULT_EXPR_PATH.to_expr_path()
                ))
            ),
        ),
    ];

    match UrlFileParam::from_url(request.url()) {
        Err(err) => HttpStreamingResponse {
            status_code: 400,
            headers,
            body: ByteBuf::from(err.as_bytes()),
            ..Default::default()
        },
        Ok(param) => {
            let id = if let Some(hash) = param.hash {
                store::fs::get_file_id(&hash).unwrap_or_default()
            } else {
                param.file
            };

            match store::fs::get_file(id) {
                None => HttpStreamingResponse {
                    status_code: 404,
                    headers,
                    body: ByteBuf::from("file not found".as_bytes()),
                    ..Default::default()
                },
                Some(file) => {
                    if !file.read_by_hash(&param.token) {
                        let canister = ic_cdk::id();
                        let ctx = match store::state::with(|s| {
                            s.read_permission(
                                ic_cdk::caller(),
                                &canister,
                                param.token,
                                ic_cdk::api::time() / SECONDS,
                            )
                        }) {
                            Ok(ctx) => ctx,
                            Err((status_code, err)) => {
                                return HttpStreamingResponse {
                                    status_code,
                                    headers,
                                    body: ByteBuf::from(err.as_bytes()),
                                    ..Default::default()
                                };
                            }
                        };

                        if file.status < 0 && ctx.role < store::Role::Auditor {
                            return HttpStreamingResponse {
                                status_code: 403,
                                headers,
                                body: ByteBuf::from("file archived".as_bytes()),
                                ..Default::default()
                            };
                        }

                        if !permission::check_file_read(&ctx.ps, &canister, id, file.parent) {
                            return HttpStreamingResponse {
                                status_code: 403,
                                headers,
                                body: ByteBuf::from("permission denied".as_bytes()),
                                ..Default::default()
                            };
                        }
                    }

                    if file.size != file.filled {
                        return HttpStreamingResponse {
                            status_code: 422,
                            headers,
                            body: ByteBuf::from("file not fully uploaded".as_bytes()),
                            ..Default::default()
                        };
                    }

                    let etag = file
                        .hash
                        .as_ref()
                        .map(|hash| BASE64.encode(hash.as_ref()))
                        .unwrap_or_default();

                    headers.push(("accept-ranges".to_string(), "bytes".to_string()));
                    if !etag.is_empty() {
                        headers.push(("etag".to_string(), format!("\"{}\"", etag)));
                    }
                    headers[0].1 = if file.content_type.is_empty() {
                        OCTET_STREAM.to_string()
                    } else {
                        file.content_type.clone()
                    };

                    if request.method() == "HEAD" {
                        headers.push(("content-length".to_string(), file.size.to_string()));
                        headers.push((
                            "cache-control".to_string(),
                            "max-age=2592000, public".to_string(),
                        ));

                        let filename = if param.inline {
                            ""
                        } else if let Some(ref name) = param.name {
                            name
                        } else {
                            &file.name
                        };

                        headers.push((
                            "content-disposition".to_string(),
                            content_disposition(filename),
                        ));

                        return HttpStreamingResponse {
                            status_code: 200,
                            headers,
                            body: ByteBuf::new(),
                            ..Default::default()
                        };
                    }

                    if let Some(range_req) = detect_range(request.headers(), file.size, &etag) {
                        match range_req {
                            Err(err) => {
                                return HttpStreamingResponse {
                                    status_code: 416,
                                    headers,
                                    body: ByteBuf::from(err.to_bytes()),
                                    ..Default::default()
                                };
                            }
                            Ok(range) => {
                                return range_response(headers, id, file, range);
                            }
                        }
                    }

                    let filename = if param.inline {
                        ""
                    } else if let Some(ref name) = param.name {
                        name
                    } else {
                        &file.name
                    };

                    headers.push((
                        "content-disposition".to_string(),
                        content_disposition(filename),
                    ));

                    // return all chunks for small file
                    let (chunk_index, body) = if file.size <= MAX_FILE_SIZE_PER_CALL {
                        (
                            file.chunks.saturating_sub(1),
                            store::fs::get_full_chunks(id)
                                .map(ByteBuf::from)
                                .unwrap_or_default(),
                        )
                    } else {
                        // return first chunk for large file
                        (
                            0,
                            store::fs::get_chunk(id, 0)
                                .map(|chunk| chunk.1)
                                .unwrap_or_default(),
                        )
                    };

                    let streaming_strategy = create_strategy(StreamingCallbackToken {
                        id,
                        chunk_index,
                        chunks: file.chunks,
                        token: None, // TODO: access token for callback
                    });

                    // small file
                    if streaming_strategy.is_none() {
                        headers.push(("content-length".to_string(), body.len().to_string()));
                        headers.push((
                            "cache-control".to_string(),
                            "max-age=2592000, public".to_string(),
                        ));
                    }

                    HttpStreamingResponse {
                        status_code: 200,
                        headers,
                        body,
                        streaming_strategy,
                        upgrade: None,
                    }
                }
            }
        }
    }
}

#[ic_cdk::query(hidden = true)]
fn http_request_streaming_callback(token: StreamingCallbackToken) -> StreamingCallbackHttpResponse {
    match store::fs::get_chunk(token.id, token.chunk_index) {
        None => ic_cdk::trap("chunk not found"),
        Some(chunk) => StreamingCallbackHttpResponse {
            body: chunk.1,
            token: token.next(),
        },
    }
}

fn detect_range(
    headers: &[(String, String)],
    full_length: u64,
    etag: &str,
) -> Option<Result<(u64, u64), String>> {
    let range = headers.iter().find_map(|(name, value)| {
        if name.to_lowercase() == "range" {
            Some(Range::from_str(value))
        } else {
            None
        }
    });

    match range {
        None => None,
        Some(Err(err)) => Some(Err(err.to_string())),
        Some(Ok(Range::Unregistered(_, _))) => {
            Some(Err("invalid range, custom range not support".to_string()))
        }
        Some(Ok(Range::Bytes(brs))) => {
            if brs.len() != 1 {
                return Some(Err(
                    "invalid range, multiple byte ranges not support".to_string()
                ));
            }

            let mut range = match brs[0].to_satisfiable_range(full_length) {
                None => return Some(Err("invalid range, out of range".to_string())),
                Some(range) => range,
            };

            if range.1 + 1 - range.0 > MAX_FILE_SIZE_PER_CALL {
                range = (range.0, range.0 + MAX_FILE_SIZE_PER_CALL - 1);
            }

            let if_range = headers.iter().find_map(|(name, value)| {
                if name.to_lowercase() == "if-range" {
                    Some(IfRange::parse_header(&Raw::from(value.as_str())))
                } else {
                    None
                }
            });

            match if_range {
                None => Some(Ok(range)),
                Some(Err(err)) => Some(Err(err.to_string())),
                Some(Ok(IfRange::Date(_))) => Some(Err(
                    "invalid if-range value, date value not support".to_string(),
                )),
                Some(Ok(IfRange::EntityTag(tag))) => {
                    if tag.tag() == etag {
                        Some(Ok(range))
                    } else {
                        Some(Err("invalid if-range value, etag not match".to_string()))
                    }
                }
            }
        }
    }
}

fn range_response(
    mut headers: Vec<(String, String)>,
    id: u32,
    metadata: store::FileMetadata,
    (start, end): (u64, u64),
) -> HttpStreamingResponse {
    let chunk_index = start / CHUNK_SIZE as u64;
    let chunk_offset = (start % CHUNK_SIZE as u64) as usize;
    let chunk_end = end / CHUNK_SIZE as u64;
    let end_offset = (end % CHUNK_SIZE as u64) as usize;

    let mut body = ByteBuf::with_capacity((end + 1 - start) as usize);
    for i in chunk_index..=chunk_end {
        let chunk = store::fs::get_chunk(id, i as u32)
            .map(|chunk| chunk.1)
            .unwrap_or_default();
        let start = if i == chunk_index { chunk_offset } else { 0 };
        let end = if i == chunk_end {
            end_offset
        } else {
            CHUNK_SIZE as usize - 1
        };

        if end >= chunk.len() {
            return HttpStreamingResponse {
                status_code: 416,
                headers,
                body: ByteBuf::from(format!("invalid range at chunk {i}").to_bytes()),
                ..Default::default()
            };
        }

        body.extend_from_slice(&chunk[start..=end]);
    }

    headers[0].1 = if metadata.content_type.is_empty() {
        OCTET_STREAM.to_string()
    } else {
        metadata.content_type.clone()
    };
    headers.push((
        "content-disposition".to_string(),
        content_disposition(&metadata.name),
    ));
    headers.push(("content-length".to_string(), body.len().to_string()));
    headers.push((
        "content-range".to_string(),
        ContentRangeSpec::Bytes {
            range: Some((start, end)),
            instance_length: Some(metadata.size),
        }
        .to_string(),
    ));

    HttpStreamingResponse {
        status_code: 206,
        headers,
        body,
        upgrade: None,
        streaming_strategy: None,
    }
}

fn content_disposition(filename: &str) -> String {
    if filename.is_empty() {
        return ContentDisposition {
            disposition: DispositionType::Inline,
            parameters: vec![],
        }
        .to_string();
    }

    let filename = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    ContentDisposition {
        disposition: DispositionType::Attachment,
        parameters: vec![DispositionParam::Filename(
            Charset::Ext("UTF-8".to_owned()),
            None,
            filename.as_bytes().to_vec(),
        )],
    }
    .to_string()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_content_disposition() {
        assert_eq!(content_disposition(""), "inline");
        assert_eq!(
            content_disposition("统计数据.txt"),
            "attachment; filename*=UTF-8''%E7%BB%9F%E8%AE%A1%E6%95%B0%E6%8D%AE.txt",
        );
        assert_eq!(
            content_disposition("/统计数据.txt"),
            "attachment; filename*=UTF-8''%E7%BB%9F%E8%AE%A1%E6%95%B0%E6%8D%AE.txt",
        );
        assert_eq!(
            content_disposition("./test.txt"),
            "attachment; filename=\"test.txt\"",
        );
    }
}
