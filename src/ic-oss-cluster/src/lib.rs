use sha3::{Digest, Sha3_256};

#[ic_cdk::query]
fn sha_256() -> String {
    let start = ic_cdk::api::instruction_counter();
    let mut hasher = sha2::Sha256::new();
    let data = [0u8; 1024];
    for _ in 0..(1024 * 10) {
        hasher.update(data);
    }
    let _: [u8; 32] = hasher.finalize().into();
    let end = ic_cdk::api::instruction_counter();

    format!("Hello, {}!", end - start)
}

#[ic_cdk::query]
fn sha3_256() -> String {
    let start = ic_cdk::api::instruction_counter();
    let mut hasher = Sha3_256::new();
    let data = [0u8; 1024];
    for _ in 0..(1024 * 10) {
        hasher.update(data);
    }
    let _: [u8; 32] = hasher.finalize().into();
    let end = ic_cdk::api::instruction_counter();

    format!("Hello, {}!", end - start)
}

ic_cdk::export_candid!();
