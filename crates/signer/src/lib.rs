use k256::SecretKey;
use tonic_primitives::{sign_message, Address, PrimitiveSignature, B256};

#[derive(Clone, Copy, Debug)]
pub struct Signer {
    secret: B256,
    address: Address,
}

impl Signer {
    pub fn new(secret: [u8; 32]) -> Self {
        let secret_key =
            SecretKey::from_slice(&secret).expect("Exactly 32 bytes secret can not fail");
        Self {
            secret: secret.into(),
            address: Address::from_private_key(&secret_key.into()),
        }
    }

    pub fn from_bytes(secret: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        Ok(Self::new(secret.as_ref().try_into()?))
    }

    pub fn from_str(secret: impl AsRef<str>) -> anyhow::Result<Self> {
        Ok(Self::from_bytes(hex::decode(secret.as_ref())?)?)
    }

    pub fn sign_prehashed(&self, prehash: B256) -> PrimitiveSignature {
        sign_message(self.secret, prehash)
            .expect("Prehash signing should not fail")
            .into()
    }

    #[inline]
    pub fn address(&self) -> Address {
        self.address
    }

    #[cfg(feature = "test-helpers")]
    pub fn random() -> Self {
        let secret: [u8; 32] = rand::random();
        Self::new(secret)
    }
}
