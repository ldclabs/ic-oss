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
