use candid::Nat;
use num_traits::cast::ToPrimitive;

pub mod bucket;
pub mod cluster;
pub mod file;

pub fn format_error<T>(err: T) -> String
where
    T: std::fmt::Debug,
{
    format!("{:?}", err)
}

pub fn crc32_with_initial(initial: u32, data: &[u8]) -> u32 {
    let mut crc32 = crc32fast::Hasher::new_with_initial(initial);
    crc32.update(data);
    crc32.finalize()
}

pub fn nat_to_u64(nat: &Nat) -> u64 {
    nat.0.to_u64().unwrap_or(0)
}

pub fn bytes32_from_hex(s: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(s).map_err(|_| format!("failed to decode hex: {}", s))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}
