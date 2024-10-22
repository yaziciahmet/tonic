#[derive(Debug)]
pub struct Signature {
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
}

impl Signature {
    pub fn r(&self) -> &[u8; 32] {
        &self.r
    }

    pub fn s(&self) -> &[u8; 32] {
        &self.s
    }

    pub fn v(&self) -> u8 {
        self.v
    }
}
