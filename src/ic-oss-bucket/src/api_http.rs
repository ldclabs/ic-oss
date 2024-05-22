use candid::{define_function, CandidType};
use hyperx::header::{Charset, ContentDisposition, DispositionParam, DispositionType};
use ic_oss_types::file::UrlFileParam;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use std::path::Path;

use crate::store;

#[derive(CandidType, Deserialize, Clone)]
pub struct HeaderField(pub String, pub String);

#[derive(CandidType, Deserialize, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String, // url path
    pub headers: Vec<HeaderField>,
    pub body: ByteBuf,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<HeaderField>,
    pub body: ByteBuf,
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

fn create_strategy(arg: StreamingCallbackToken) -> Option<StreamingStrategy> {
    arg.next().map(|token| StreamingStrategy::Callback {
        token,
        callback: CallbackFunc::new(ic_cdk::id(), "http_request_streaming_callback".to_string()),
    })
}

// request url example:
// https://bwwuq-byaaa-aaaan-qmk4q-cai.raw.icp0.io/f/1
// http://bkyz2-fmaaa-aaaaa-qaaaq-cai.localhost:4943/f/1 // download file by id 1
// http://bkyz2-fmaaa-aaaaa-qaaaq-cai.localhost:4943/h/8546ffa4296a6960e9e64e95de178d40c231a0cd358a65477bc56a105dda1c1d //download file by hash 854...
// TODO: 1. support range request; 2. token verification; 3. ICP verification header
#[ic_cdk::query]
fn http_request(request: HttpRequest) -> HttpResponse {
    match UrlFileParam::from_url(&request.url) {
        Err(err) => HttpResponse {
            body: ByteBuf::from(err.as_bytes()),
            status_code: 400,
            headers: vec![],
            streaming_strategy: None,
        },
        Ok(param) => {
            let id = if let Some(hash) = param.hash {
                store::fs::get_file_id(&hash).unwrap_or_default()
            } else {
                param.file
            };

            match store::fs::get_file(id) {
                None => HttpResponse {
                    body: ByteBuf::from("file not found".as_bytes()),
                    status_code: 404,
                    headers: vec![],
                    streaming_strategy: None,
                },
                Some(metadata) => HttpResponse {
                    body: store::fs::get_chunk(id, 0)
                        .map(|chunk| chunk.1)
                        .unwrap_or_default(),
                    status_code: 200,
                    headers: vec![
                        HeaderField("content-type".to_string(), metadata.content_type.clone()),
                        HeaderField("accept-ranges".to_string(), "bytes".to_string()),
                        HeaderField(
                            "content-disposition".to_string(),
                            content_disposition(&metadata.name),
                        ),
                        HeaderField(
                            "cache-control".to_string(),
                            "max-age=2592000, public".to_string(),
                        ),
                    ],
                    streaming_strategy: create_strategy(StreamingCallbackToken {
                        id,
                        chunk_index: 0,
                        chunks: metadata.chunks,
                        token: param.token,
                    }),
                },
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
