use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::address::Address;
use crate::crypto::verify_with_context;

/// `Signature` is a simple byte wrapper for easy serde operations and type conversions.
/// It doesn't validate any Ed25519 signature properties until explicitly verified.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Signature([u8; 64]);

impl Signature {
    pub const SIZE: usize = 64;

    /// Create `Signature` from bytes.
    #[inline(always)]
    pub fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(bytes)
    }

    /// Create `Signature` from slice. Expects slice to be of size 64.
    #[inline(always)]
    pub fn from_slice(slice: &[u8]) -> anyhow::Result<Self> {
        Ok(Self::from_bytes(slice.try_into()?))
    }

    /// Create `Signature` from 0x-prefixed hex string
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        if !s.starts_with("0x") {
            return Err(anyhow::anyhow!("Signature must have 0x-prefix"));
        }

        let mut bytes = [0; Self::SIZE];
        hex::decode_to_slice(&s[2..], &mut bytes)?;
        Ok(Self(bytes))
    }

    /// Get signature bytes.
    #[inline(always)]
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.0
    }

    /// Encode `Signature` as 0x-prefixed hex char bytes
    fn encode_0x_prefixed(&self) -> [u8; Self::SIZE * 2 + 2] {
        let mut bytes = [0u8; Self::SIZE * 2 + 2];
        bytes[0] = b'0';
        bytes[1] = b'x';

        hex::encode_to_slice(self.to_bytes(), &mut bytes[2..])
            .expect("Size precalculated, encode can not fail");
        bytes
    }

    /// Verifiy the signer of the signature from the message with context
    pub fn verify_with_context(
        &self,
        address: Address,
        message: &[u8],
        context: &[u8],
    ) -> anyhow::Result<()> {
        Ok(verify_with_context(
            &address.try_into()?,
            message,
            &self.0,
            context,
        )?)
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.encode_0x_prefixed();
        let s = unsafe { std::str::from_utf8_unchecked(&bytes) };
        f.write_str(s)
    }
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let bytes = self.encode_0x_prefixed();
            let s = unsafe { std::str::from_utf8_unchecked(&bytes) };
            serializer.serialize_str(s)
        } else {
            serializer.serialize_bytes(&self.to_bytes())
        }
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s: &str = Deserialize::deserialize(deserializer)?;
            Self::from_str(s).map_err(|err| de::Error::custom(err))
        } else {
            let slice: &[u8] = Deserialize::deserialize(deserializer)?;
            Self::from_slice(slice).map_err(|err| de::Error::custom(err))
        }
    }
}

#[cfg(test)]
mod tests {
    use borsh::{BorshDeserialize, BorshSerialize};
    use serde::{Deserialize, Serialize};

    use crate::{
        address::Address,
        crypto::{generate_key, sign_with_context},
    };

    use super::Signature;

    #[test]
    fn hex_signature() {
        let s = "0x01010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101".to_string();
        let signature = Signature::from_str(&s).unwrap();
        assert_eq!(s, signature.to_string());
        assert_eq!([1; 64], signature.to_bytes());

        // 63-bytes
        let s = "0x010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101";
        assert!(matches!(Signature::from_str(s), Err(_)));
        // 65-bytes
        let s = "0x0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101";
        assert!(matches!(Signature::from_str(s), Err(_)));
        // 64-bytes, no 0x-prefix
        let s = "01010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101";
        assert!(matches!(Signature::from_str(s), Err(_)));
    }

    #[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
    struct TestStruct {
        signature: Signature,
    }

    #[test]
    fn serde_signature() {
        let test_struct = TestStruct {
            signature: Signature::from_bytes([1; 64]),
        };

        let json = serde_json::to_string(&test_struct).unwrap();
        assert_eq!(
            &json,
            r#"{"signature":"0x01010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"}"#
        );

        let test_struct_deserialized: TestStruct = serde_json::from_str(&json).unwrap();
        assert_eq!(test_struct, test_struct_deserialized);

        // Missing 0x-prefix should fail
        assert!(matches!(
            serde_json::from_str::<TestStruct>(
                r#"{"signature":"01010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"}"#,
            ),
            Err(_)
        ));
    }

    #[test]
    fn borsh_signature() {
        let test_struct = TestStruct {
            signature: Signature::from_bytes([1; 64]),
        };
        assert_eq!(&borsh::to_vec(&test_struct).unwrap(), &[1; 64]);
        assert_eq!(test_struct, borsh::from_slice(&[1; 64]).unwrap());
    }

    #[test]
    fn verify_with_context() {
        let signing_key = generate_key();
        let message = &[1, 2, 3];
        let context = b"TonicMainnet";

        let signature_bytes = sign_with_context(&signing_key, message, context);

        let signature = Signature::from_bytes(signature_bytes);
        signature
            .verify_with_context(Address::from(signing_key.verifying_key()), message, context)
            .unwrap();
    }
}
