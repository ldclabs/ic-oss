use anyhow::Result;
use ic_agent::{agent::http_transport::ReqwestTransport, export::Principal, Agent};
use ic_http_certification::{HttpRequest, HttpResponse};
use ic_response_verification::verify_request_response_pair;
use ic_utils::{
    call::SyncCall,
    interfaces::{http_request::HeaderField, HttpRequestCanister},
};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_CERT_TIME_OFFSET_NS: u128 = 300_000_000_000;

pub async fn create_agent(url: &str) -> Result<Agent> {
    let transport = ReqwestTransport::create(url)?;

    let agent = Agent::builder().with_transport(transport).build()?;
    agent.fetch_root_key().await?;

    Ok(agent)
}

fn get_current_time_in_ns() -> u128 {
    let start = SystemTime::now();

    start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos()
}

#[tokio::main]
async fn main() -> Result<()> {
    const REPLICA_ADDRESS: &str = "https://icp-api.io";
    const CANISTER_ID: &str = "mmrxu-fqaaa-aaaap-ahhna-cai";

    const PATH: &str = "/f/1";
    const HTTP_METHOD: &str = "GET";
    let headers: Vec<HeaderField> = vec![];

    let agent = create_agent(REPLICA_ADDRESS).await?;
    let root_key = agent.read_root_key();
    let canister_id = Principal::from_text(CANISTER_ID)?;
    let canister_interface = HttpRequestCanister::create(&agent, canister_id);

    let res_headers: Vec<HeaderField> = vec![HeaderField(
        "ic-certificateexpression".into(),
        "default_certification(ValidationArgs{no_certification:Empty{}})".into(),
    ),
    HeaderField(
        "ic-certificate".into(),
         "certificate=:2dn3omR0cmVlgwGDAYMBggRYICZ9EIuCUUnIiSJ/IgyiAROM5iHDHHw88XQRfnAVEQu3gwJIY2FuaXN0ZXKDAYMCSgAAAAAB4DnaAQGDAYMBgwJOY2VydGlmaWVkX2RhdGGCA1ggVezDHD6AR/wVjyDqCqBgmqzHtX4G6kDcQYYCcqfWQpGCBFggxgNY0FQtNvK3bicmcmg2eLrIqGyDGeidxu33H+AamlyCBFggGS1PJQt++2k00JC5uF9ousT8Gp+DRVBqBV/lm90NJD2CBFggr6IHC8ZsaRqKnCx5qMJZTDrk1xRt8aN1/DxvGnsQsgqCBFgg2v7H05tMdK+IPGYiqV6guIvfdlWxJum3jKjFLgz07WqDAYIEWCBG1/w6B/AZF5XvJEafIY82HLagfB+rpcPsgOhFSikLZYMCRHRpbWWCA0ngpdGct6+I6hdpc2lnbmF0dXJlWDCkbc0En6sNAYFzh9spsxlCLFgJYvo6sFG4IyQpCfXzIJxRZZclPs78FdF8eAgcbAw=:, tree=:gwJJaHR0cF9leHBygwJDPCo+gwJYIMMautvQsFn51GT9bfTani3Ah659C0BGjTNyJtQTszcjggNA:, expr_path=:gmlodHRwX2V4cHJjPCo+:, version=2".into()
        )];
    let (response,) = canister_interface
        .http_request(HTTP_METHOD, PATH, res_headers, &[], Some(&2))
        .call()
        .await?;

    let result = verify_request_response_pair(
        HttpRequest {
            method: HTTP_METHOD.to_string(),
            url: PATH.to_string(),
            headers: headers
                .iter()
                .map(|HeaderField(k, v)| (k.clone().into_owned(), v.clone().into_owned()))
                .collect(),
            body: vec![],
        },
        HttpResponse {
            status_code: response.status_code,
            headers: response
                .headers
                .iter()
                .map(|HeaderField(k, v)| (k.clone().into_owned(), v.clone().into_owned()))
                .collect(),
            body: response.body.clone(),
            upgrade: None,
        },
        canister_id.as_slice(),
        get_current_time_in_ns(),
        MAX_CERT_TIME_OFFSET_NS,
        root_key.as_slice(),
        2,
    );
    println!("Verification result: {:?}", result);
    // println!("Response: {:?}", response);

    Ok(())
}
