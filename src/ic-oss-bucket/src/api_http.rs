use candid::{define_function, CandidType};
use ic_oss_types::file::UrlFileParam;
use serde::Deserialize;
use serde_bytes::ByteBuf;

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
// https://bwwuq-byaaa-aaaan-qmk4q-cai.raw.icp0.io/f1
// http://bkyz2-fmaaa-aaaaa-qaaaq-cai.localhost:4943/f1
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
        Ok(param) => match store::fs::get_file(param.file) {
            None => HttpResponse {
                body: ByteBuf::from("file not found".as_bytes()),
                status_code: 404,
                headers: vec![],
                streaming_strategy: None,
            },
            Some(metadata) => {
                // todo: escape filename
                let filename = metadata.name.strip_prefix('/').unwrap_or(&metadata.name);
                let filename = format!("attachment; filename={}", filename);
                HttpResponse {
                    body: ByteBuf::from(
                        store::fs::get_chunk(param.file, 0)
                            .map(|chunk| chunk.0)
                            .unwrap_or_default(),
                    ),
                    status_code: 200,
                    headers: vec![
                        HeaderField("content-type".to_string(), metadata.content_type.clone()),
                        HeaderField("accept-ranges".to_string(), "bytes".to_string()),
                        HeaderField("content-disposition".to_string(), filename),
                        HeaderField(
                            "cache-control".to_string(),
                            "max-age=2592000, public".to_string(),
                        ),
                    ],
                    streaming_strategy: create_strategy(StreamingCallbackToken {
                        id: param.file,
                        chunk_index: 0,
                        chunks: metadata.chunks,
                        token: param.token,
                    }),
                }
            }
        },
    }
}

#[ic_cdk::query]
fn http_request_streaming_callback(token: StreamingCallbackToken) -> StreamingCallbackHttpResponse {
    match store::fs::get_chunk(token.id, token.chunk_index) {
        None => ic_cdk::trap("chunk not found"),
        Some(chunk) => StreamingCallbackHttpResponse {
            body: ByteBuf::from(chunk.0),
            token: token.next(),
        },
    }
}
