use secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
pub use secp256k1::{Error, Keypair, PublicKey, SecretKey};
use secp256k1::{Message, SECP256K1};

pub fn sign_prehash(secret_key: &SecretKey, prehash: [u8; 32]) -> ([u8; 64], u8) {
    let (recid, signature) = SECP256K1
        .sign_ecdsa_recoverable(&Message::from_digest(prehash), &secret_key)
        .serialize_compact();
    (signature, i32::from(recid) as u8)
}

pub fn recover_from_prehash(
    prehash: [u8; 32],
    signature: [u8; 64],
    recid: u8,
) -> Result<PublicKey, Error> {
    let signature =
        RecoverableSignature::from_compact(&signature, RecoveryId::try_from(recid as i32)?)?;
    SECP256K1.recover_ecdsa(&Message::from_digest(prehash), &signature)
}

#[cfg(feature = "k256-rand")]
pub fn generate_keypair() -> Keypair {
    Keypair::new_global(&mut secp256k1::rand::thread_rng())
}

pub trait ToPublicKey {
    fn to_public_key(&self) -> PublicKey;
}

impl ToPublicKey for SecretKey {
    fn to_public_key(&self) -> PublicKey {
        self.public_key(SECP256K1)
    }
}
