use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use candid::{define_function, CandidType};
use hyperx::header::{Charset, ContentDisposition, DispositionParam, DispositionType};
use ic_http_certification::{HeaderField, HttpRequest};
use ic_oss_types::{
    file::{UrlFileParam, MAX_FILE_SIZE_PER_CALL},
    to_cbor_bytes,
};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use std::path::Path;

use crate::store;

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
// https://mmrxu-fqaaa-aaaap-ahhna-cai.raw.icp0.io/f/1
// http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/f/1 // download file by id 1
// http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/h/8546ffa4296a6960e9e64e95de178d40c231a0cd358a65477bc56a105dda1c1d //download file by hash 854...
// TODO: 1. support range request; 2. token verification; 3. ICP verification header
#[ic_cdk::query]
fn http_request(request: HttpRequest) -> HttpStreamingResponse {
    match UrlFileParam::from_url(&request.url) {
        Err(err) => HttpStreamingResponse {
            status_code: 400,
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
                    body: ByteBuf::from("file not found".as_bytes()),
                    ..Default::default()
                },
                Some(metadata) => {
                    if metadata.size != metadata.filled {
                        return HttpStreamingResponse {
                            status_code: 404,
                            body: ByteBuf::from("file not fully uploaded".as_bytes()),
                            ..Default::default()
                        };
                    }

                    let (chunk_index, body) = if metadata.size <= MAX_FILE_SIZE_PER_CALL {
                        (
                            metadata.chunks.saturating_sub(1),
                            store::fs::get_full_chunks(id)
                                .map(ByteBuf::from)
                                .unwrap_or_default(),
                        )
                    } else {
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
                        chunks: metadata.chunks,
                        token: param.token,
                    });

                    let witness = store::state::http_tree_with(|t| {
                        t.witness(&store::state::DEFAULT_CERT_ENTRY, &request.url)
                            .expect("get witness failed")
                    });
                    let certified_data =
                        ic_cdk::api::data_certificate().expect("no data certificate available");

                    let mut headers = vec![
                        (
                            "content-type".to_string(),
                            if metadata.content_type.is_empty() {
                                OCTET_STREAM.to_string()
                            } else {
                                metadata.content_type.clone()
                            },
                        ),
                        ("x-content-type-options".to_string(), "nosniff".to_string()),
                        (
                            "content-disposition".to_string(),
                            content_disposition(&metadata.name),
                        ),
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

                    if let Some(hash) = metadata.hash {
                        headers.push(("etag".to_string(), BASE64.encode(hash)));
                    }

                    // small file
                    if streaming_strategy.is_none() {
                        headers.push(("content-length".to_string(), body.len().to_string()));
                        headers.push((
                            "cache-control".to_string(),
                            "max-age=2592000, public".to_string(),
                        ));
                    } else {
                        // headers.push(("accept-ranges".to_string(), "bytes".to_string())); // TODO: support range request
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

#[ic_cdk::query]
fn http_request_streaming_callback(token: StreamingCallbackToken) -> StreamingCallbackHttpResponse {
    match store::fs::get_chunk(token.id, token.chunk_index) {
        None => ic_cdk::trap("chunk not found"),
        Some(chunk) => StreamingCallbackHttpResponse {
            body: chunk.1,
            token: token.next(),
        },
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
