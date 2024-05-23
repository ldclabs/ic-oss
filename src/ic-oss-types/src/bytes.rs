use serde_bytes::ByteBuf;
use std::ops::Deref;

pub struct Bytes32(pub [u8; 32]);

impl Deref for Bytes32 {
    type Target = [u8; 32];
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Bytes32> for Vec<u8> {
    fn from(value: Bytes32) -> Self {
        value.0.to_vec()
    }
}

impl From<Bytes32> for ByteBuf {
    fn from(value: Bytes32) -> Self {
        ByteBuf::from(value.0.to_vec())
    }
}

impl TryFrom<&[u8]> for Bytes32 {
    type Error = String;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != 32 {
            return Err(format!("expected 32 bytes, got {}", value.len()));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(value);
        Ok(Bytes32(bytes))
    }
}

impl TryFrom<Vec<u8>> for Bytes32 {
    type Error = String;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let len = value.len();
        let bytes: [u8; 32] = value
            .try_into()
            .map_err(|_| format!("expected 32 bytes, got {}", len))?;
        Ok(Bytes32(bytes))
    }
}

impl TryFrom<&str> for Bytes32 {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let data = hex::decode(value).map_err(|_| format!("failed to decode hex: {}", value))?;
        Self::try_from(data.as_slice())
    }
}

impl TryFrom<ByteBuf> for Bytes32 {
    type Error = String;

    fn try_from(value: ByteBuf) -> Result<Self, Self::Error> {
        Self::try_from(value.to_vec())
    }
}

impl TryFrom<&ByteBuf> for Bytes32 {
    type Error = String;

    fn try_from(value: &ByteBuf) -> Result<Self, Self::Error> {
        Self::try_from(value.as_slice())
    }
}
