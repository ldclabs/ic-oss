use candid::Nat;
use ciborium::into_writer;
use num_traits::cast::ToPrimitive;
use serde::Serialize;

pub mod bucket;
pub mod cluster;
pub mod file;

mod bytes;
pub use bytes::*;

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

// to_cbor_bytes returns the CBOR encoding of the given object that implements the Serialize trait.
pub fn to_cbor_bytes(obj: &impl Serialize) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    into_writer(obj, &mut buf).expect("failed to encode in CBOR format");
    buf
}
