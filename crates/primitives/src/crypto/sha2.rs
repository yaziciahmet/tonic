use sha2::{Digest, Sha256};

/// Sha256 the message
pub fn sha256(message: &[u8]) -> [u8; 32] {
    Sha256::digest(message).into()
}
