use rand::RngCore;

pub mod agent;
pub mod client;

pub use agent::*;
pub use client::*;

/// Generates an array of random bytes of specified size.
///
/// # Examples
/// ```
/// use ic_object_store::rand_bytes;
///
/// let random_bytes: [u8; 32] = rand_bytes();
/// assert_eq!(random_bytes.len(), 32);
/// ```
pub fn rand_bytes<const N: usize>() -> [u8; N] {
    let mut rng = rand::rng();
    let mut bytes = [0u8; N];
    rng.fill_bytes(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
