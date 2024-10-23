use secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
pub use secp256k1::PublicKey;
use secp256k1::{Message, SecretKey, SECP256K1};

/// Recovers the signer from the signature and message hash
pub fn recover_ecdsa(sig: &[u8; 65], msg: &[u8; 32]) -> Result<PublicKey, secp256k1::Error> {
    let sig =
        RecoverableSignature::from_compact(&sig[0..64], RecoveryId::try_from(sig[64] as i32)?)?;

    SECP256K1.recover_ecdsa(&Message::from_digest(*msg), &sig)
}

/// Signs message with the given secret key.
/// Returns the corresponding signature.
pub fn sign_message(secret: &[u8; 32], msg: &[u8; 32]) -> [u8; 65] {
    let secret =
        SecretKey::from_byte_array(secret).expect("32 bytes array to secret key can not fail");
    let (rec_id, sig) = SECP256K1
        .sign_ecdsa_recoverable(&Message::from_digest(*msg), &secret)
        .serialize_compact();

    let mut full_sig = [0u8; 65];
    full_sig[0..64].copy_from_slice(&sig);
    full_sig[64] = rec_id as u8;

    full_sig
}

#[cfg(feature = "test-helpers")]
pub fn random_keypair() -> (SecretKey, PublicKey) {
    secp256k1::generate_keypair(&mut rand::thread_rng())
}

#[cfg(test)]
mod tests {
    use crate::secp256k1::random_keypair;

    use super::{recover_ecdsa, sign_message};

    #[test]
    fn sign() {
        let secret = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];

        let hash = b"11111111111111111111111111111111";
        let signature = sign_message(&secret, hash);
        assert_eq!(
            signature,
            [
                68, 71, 243, 174, 34, 231, 134, 222, 51, 44, 197, 237, 53, 87, 235, 230, 110, 224,
                201, 1, 176, 42, 83, 83, 253, 205, 65, 19, 28, 78, 152, 129, 64, 48, 204, 126, 191,
                39, 247, 236, 2, 73, 198, 208, 12, 166, 122, 25, 240, 58, 144, 133, 25, 229, 250,
                167, 100, 235, 141, 224, 215, 251, 195, 50, 1,
            ]
        );
    }

    #[test]
    fn sign_then_recover() {
        let (secret, public) = random_keypair();

        let hash = b"00000000000000000000000000000000";
        let signature = sign_message(&secret.secret_bytes(), hash);

        let recovered = recover_ecdsa(&signature, hash).expect("no error in recover ecdsa");
        assert_eq!(recovered, public);
    }
}
