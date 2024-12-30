use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::crypto::{sha256, PublicKey};

/// `Address` is a simple byte wrapper for easy serde operations and type conversions.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Address([u8; 24]);

impl Address {
    pub const SIZE: usize = 24;

    /// Create an `Address` from the given bytes.
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(bytes)
    }

    /// Create an `Address` from the given slice. Expects 24 bytes.
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
        let keypair = crate::crypto::generate_keypair();
        keypair.public_key().into()
    }
}

impl From<PublicKey> for Address {
    fn from(public_key: PublicKey) -> Self {
        let hash = sha256(&public_key.serialize());
        Self(hash[8..].try_into().expect("Has exactly 24 bytes"))
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
            Self::from_str(s).map_err(de::Error::custom)
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
        let s = "0x010101010101010101010101010101010101010101010101".to_string();
        let addr = Address::from_str(&s).unwrap();
        assert_eq!(s, addr.to_string());
        assert_eq!([1; 24], addr.to_bytes());

        // 23-bytes
        let s = "0x0101010101010101010101010101010101010101010101";
        assert!(Address::from_str(s).is_err());
        // 25-bytes
        let s = "0x01010101010101010101010101010101010101010101010101";
        assert!(Address::from_str(s).is_err());
        // 24-bytes, no 0x-prefix
        let s = "010101010101010101010101010101010101010101010101";
        assert!(Address::from_str(s).is_err());
    }

    #[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
    struct TestStruct {
        address: Address,
    }

    #[test]
    fn serde_address() {
        let test_struct = TestStruct {
            address: Address::from_bytes([10; 24]),
        };

        let json = serde_json::to_string(&test_struct).unwrap();
        assert_eq!(
            &json,
            r#"{"address":"0x0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a"}"#
        );

        let test_struct_deserialized: TestStruct = serde_json::from_str(&json).unwrap();
        assert_eq!(test_struct, test_struct_deserialized);

        let test_struct_deserialized_uppercase: TestStruct = serde_json::from_str(
            r#"{"address":"0x0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A"}"#,
        )
        .unwrap();
        assert_eq!(test_struct, test_struct_deserialized_uppercase);

        // Missing 0x-prefix should fail
        assert!(serde_json::from_str::<TestStruct>(
            r#"{"address":"0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a"}"#,
        )
        .is_err());
    }

    #[test]
    fn borsh_address() {
        let test_struct = TestStruct {
            address: Address::from_bytes([10; 24]),
        };
        assert_eq!(&borsh::to_vec(&test_struct).unwrap(), &[10; 24]);
        assert_eq!(test_struct, borsh::from_slice(&[10; 24]).unwrap());
    }
}
