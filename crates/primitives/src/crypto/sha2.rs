use sha2::{Digest, Sha512, Sha512_256};

/// SHA-512-256 the message. Hashes with SHA-512 and produces 256-bit output.
pub fn sha512_256(message: &[u8]) -> [u8; 32] {
    Sha512_256::digest(message).into()
}

/// SHA-512 the message. Only used for Ed25519 signing.
pub fn sha512(message: &[u8]) -> Sha512 {
    Sha512::new_with_prefix(message)
}
