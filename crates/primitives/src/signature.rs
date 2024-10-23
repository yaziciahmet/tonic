#[derive(Debug)]
pub struct Signature([u8; 65]);

impl Signature {
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
