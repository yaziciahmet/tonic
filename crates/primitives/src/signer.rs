use crate::crypto::{sign_with_context, SigningKey};
use crate::{Address, Signature};

#[derive(Debug, Clone)]
pub struct Signer {
    signing_key: SigningKey,
    address: Address,
    context: Vec<u8>,
}

impl Signer {
    fn from_signing_key(signing_key: SigningKey, context: Vec<u8>) -> Self {
        let address = Address::from(signing_key.verifying_key());
        Self {
            signing_key,
            address,
            context,
        }
    }

    pub fn new(secret: [u8; 32], context: Vec<u8>) -> Self {
        let signing_key = SigningKey::from_bytes(&secret);
        Self::from_signing_key(signing_key, context)
    }

    pub fn from_bytes(secret: impl AsRef<[u8]>, context: Vec<u8>) -> anyhow::Result<Self> {
        Ok(Self::new(secret.as_ref().try_into()?, context))
    }

    pub fn from_str(secret: &str, context: Vec<u8>) -> anyhow::Result<Self> {
        if !secret.starts_with("0x") {
            return Err(anyhow::anyhow!("Signed secret key must have 0x-prefix"));
        }

        Self::from_bytes(hex::decode(&secret[2..])?, context)
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        let signature_bytes = sign_with_context(&self.signing_key, message, &self.context);
        Signature::from_bytes(signature_bytes)
    }

    #[inline(always)]
    pub fn address(&self) -> Address {
        self.address
    }

    #[cfg(feature = "test-helpers")]
    pub fn random() -> Self {
        Self::from_signing_key(crate::crypto::generate_key(), vec![])
    }
}

#[derive(Debug, Clone, Default)]
pub struct SignVerifier {
    context: Vec<u8>,
}

impl SignVerifier {
    /// Creates new `SignVerifier` with the given context.
    pub fn new(context: Vec<u8>) -> Self {
        Self { context }
    }

    /// Verifies that the given signature is the message signed by the address.
    /// Keep in mind that while signing the same context should be used while verifying.
    pub fn verify(
        &self,
        address: Address,
        message: &[u8],
        signature: Signature,
    ) -> anyhow::Result<()> {
        signature.verify_with_context(address, message, &self.context)
    }
}

#[cfg(test)]
mod tests {
    use super::{SignVerifier, Signer};

    #[test]
    fn sign_and_verify() {
        let signer = Signer::random();
        let message = &[1, 2, 3];

        let signature = signer.sign(message);
        SignVerifier::default()
            .verify(signer.address(), message, signature)
            .unwrap();
    }
}
