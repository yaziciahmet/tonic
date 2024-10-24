use std::array::TryFromSliceError;

#[derive(Debug)]
pub struct Signature([u8; 65]);

impl Signature {
    pub fn new(full: [u8; 65]) -> Self {
        Self(full)
    }

    pub fn try_from_slice(slice: &[u8]) -> Result<Self, TryFromSliceError> {
        Ok(Self(slice.try_into()?))
    }

    pub fn from_rsv(r: &[u8; 32], s: &[u8; 32], v: u8) -> Self {
        let mut full = [0u8; 65];
        full[0..32].copy_from_slice(r);
        full[32..64].copy_from_slice(s);
        full[64] = v;

        Self(full)
    }

    pub fn full(&self) -> &[u8; 65] {
        &self.0
    }

    pub fn r(&self) -> &[u8; 32] {
        self.0[0..32].try_into().expect("Has exactly 32 bytes")
    }

    pub fn s(&self) -> &[u8; 32] {
        self.0[32..64].try_into().expect("Has exactly 32 bytes")
    }

    pub fn v(&self) -> u8 {
        self.0[64]
    }
}
