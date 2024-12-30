use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::crypto::VerifyingKey;

/// `Address` is a simple byte wrapper for easy serde operations and type conversions.
/// It doesn't validate any Ed25519 public key properties until explicitly verified.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Address([u8; 32]);

impl Address {
    pub const SIZE: usize = 32;

    /// Create an `Address` from the given bytes.
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(bytes)
    }

    /// Create an `Address` from the given slice. Expects 32 bytes.
    #[inline(always)]
    pub fn from_slice(slice: &[u8]) -> anyhow::Result<Self> {
        Ok(Self(slice.try_into()?))
    }

    /// Create `Address` from 0x-prefixed hex string.
    #[inline(always)]
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        if !s.starts_with("0x") {
            return Err(anyhow::anyhow!("Address must have 0x-prefix"));
        }

        let mut bytes = [0; Self::SIZE];
        hex::decode_to_slice(&s[2..], &mut bytes)?;
        Ok(Self(bytes))
    }

    /// Get address bytes.
    #[inline(always)]
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.0
    }

    /// Get reference to address bytes.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Encode the address as 0x-prefixed utf8 bytes.
    fn encode_0x_prefixed(&self) -> [u8; Self::SIZE * 2 + 2] {
        let mut bytes = [0u8; Self::SIZE * 2 + 2];
        bytes[0] = b'0';
        bytes[1] = b'x';

        hex::encode_to_slice(self.0, &mut bytes[2..])
            .expect("Size precalculated, encode can not fail");
        bytes
    }

    /// Generate a random address.
    #[cfg(feature = "test-helpers")]
    pub fn random() -> Self {
        let signing_key = crate::crypto::generate_key();
        signing_key.verifying_key().into()
    }
}

impl From<VerifyingKey> for Address {
    fn from(verifying_key: VerifyingKey) -> Self {
        Self(verifying_key.to_bytes())
    }
}

impl TryFrom<Address> for VerifyingKey {
    type Error = anyhow::Error;
    fn try_from(address: Address) -> Result<Self, Self::Error> {
        VerifyingKey::from_bytes(&address.0).map_err(|err| anyhow::anyhow!(err))
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.encode_0x_prefixed();
        let s = unsafe { std::str::from_utf8_unchecked(&bytes) };
        f.write_str(s)
    }
}

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let bytes = self.encode_0x_prefixed();
            let s = unsafe { std::str::from_utf8_unchecked(&bytes) };
            serializer.serialize_str(s)
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s: &str = Deserialize::deserialize(deserializer)?;
            Self::from_str(s).map_err(|err| de::Error::custom(err))
        } else {
            let bytes: [u8; Self::SIZE] = Deserialize::deserialize(deserializer)?;
            Ok(Self(bytes))
        }
    }
}

#[cfg(test)]
mod tests {
    use borsh::{BorshDeserialize, BorshSerialize};
    use serde::{Deserialize, Serialize};

    use super::Address;

    #[test]
    fn hex_address() {
        let s = "0x0101010101010101010101010101010101010101010101010101010101010101".to_string();
        let addr = Address::from_str(&s).unwrap();
        assert_eq!(s, addr.to_string());
        assert_eq!([1; 32], addr.to_bytes());

        // 31-bytes
        let s = "0x01010101010101010101010101010101010101010101010101010101010101";
        assert!(matches!(Address::from_str(s), Err(_)));
        // 33-bytes
        let s = "0x010101010101010101010101010101010101010101010101010101010101010101";
        assert!(matches!(Address::from_str(s), Err(_)));
        // 32-bytes, no 0x-prefix
        let s = "0101010101010101010101010101010101010101010101010101010101010101";
        assert!(matches!(Address::from_str(s), Err(_)));
    }

    #[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
    struct TestStruct {
        address: Address,
    }

    #[test]
    fn serde_address() {
        let test_struct = TestStruct {
            address: Address::from_bytes([10; 32]),
        };

        let json = serde_json::to_string(&test_struct).unwrap();
        assert_eq!(
            &json,
            r#"{"address":"0x0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a"}"#
        );

        let test_struct_deserialized: TestStruct = serde_json::from_str(&json).unwrap();
        assert_eq!(test_struct, test_struct_deserialized);

        let test_struct_deserialized_uppercase: TestStruct = serde_json::from_str(
            r#"{"address":"0x0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A"}"#,
        )
        .unwrap();
        assert_eq!(test_struct, test_struct_deserialized_uppercase);

        // Missing 0x-prefix should fail
        assert!(matches!(
            serde_json::from_str::<TestStruct>(
                r#"{"address":"0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a"}"#,
            ),
            Err(_)
        ));
    }

    #[test]
    fn borsh_address() {
        let test_struct = TestStruct {
            address: Address::from_bytes([10; 32]),
        };
        assert_eq!(&borsh::to_vec(&test_struct).unwrap(), &[10; 32]);
        assert_eq!(test_struct, borsh::from_slice(&[10; 32]).unwrap());
    }
}
