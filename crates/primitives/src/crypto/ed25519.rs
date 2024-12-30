pub use ed25519_dalek::ed25519::Error;
use ed25519_dalek::ed25519::Signature;
pub use ed25519_dalek::{SigningKey, VerifyingKey};

use super::sha2::sha512;

/// Signs the message with the signing key and the provided context. Returns the signature bytes.
pub fn sign_with_context(signing_key: &SigningKey, message: &[u8], context: &[u8]) -> [u8; 64] {
    let digest = sha512(message);
    signing_key
        .sign_prehashed(digest, Some(context))
        .expect("Signing should not fail")
        .into()
}

/// Verify that signature is the result of signing of the message and the context by the verifying key.
pub fn verify_with_context(
    verifying_key: &VerifyingKey,
    message: &[u8],
    signature: &[u8; 64],
    context: &[u8],
) -> Result<(), Error> {
    let digest = sha512(message);
    verifying_key.verify_prehashed_strict(digest, Some(context), &Signature::from_bytes(signature))
}

/// Generate a cryptographically secure random Ed25519 key
#[cfg(feature = "ed25519-rand")]
pub fn generate_key() -> SigningKey {
    let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
    // Sanity check to ensure
    assert!(
        !signing_key.verifying_key().is_weak(),
        "Should not generate a signing key with a weak verifying key"
    );
    signing_key
}
