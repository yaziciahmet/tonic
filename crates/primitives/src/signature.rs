use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::address::Address;
use crate::crypto::{recover_from_prehash, sha256};

/// `Signature` is a simple byte wrapper for easy serde operations and type conversions.
/// It only verifies the recovery id and nothing else until signer is recovered explicitly.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Signature([u8; 65]);

impl Signature {
    pub const SIZE: usize = 65;

    /// Create `Signature` from bytes.
    #[inline(always)]
    pub fn from_bytes(bytes: [u8; Self::SIZE]) -> anyhow::Result<Self> {
        let v = bytes[64];
        if v <= 3 {
            Ok(Self(bytes))
        } else {
            Err(anyhow::anyhow!("Invalid signature recovery id {v}"))
        }
    }

    /// Create `Signature` from raw signature and recovery id
    #[inline(always)]
    pub fn from_parts(signature: [u8; 64], recid: u8) -> anyhow::Result<Self> {
        let mut bytes = [0; 65];
        bytes[..64].copy_from_slice(&signature);
        bytes[64] = recid;
        Self::from_bytes(bytes)
    }

    /// Create `Signature` from slice. Expects slice to be of size 65.
    #[inline(always)]
    pub fn from_slice(slice: &[u8]) -> anyhow::Result<Self> {
        Self::from_bytes(slice.try_into()?)
    }

    /// Create `Signature` from 0x-prefixed hex string
    pub fn from_hex(s: &str) -> anyhow::Result<Self> {
        if !s.starts_with("0x") {
            return Err(anyhow::anyhow!("Signature must have 0x-prefix"));
        }

        let mut bytes = [0; Self::SIZE];
        hex::decode_to_slice(&s[2..], &mut bytes)?;
        Self::from_bytes(bytes)
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

    pub fn recover(&self, message: &[u8]) -> anyhow::Result<Address> {
        let prehash = sha256(message);
        self.recover_from_prehash(prehash)
    }

    pub fn recover_from_prehash(&self, prehash: [u8; 32]) -> anyhow::Result<Address> {
        let (recid, signature) = self.0.split_last().expect("Can not be empty");
        Ok(recover_from_prehash(
            prehash,
            signature.try_into().expect("Has exactly 64 bytes"),
            *recid,
        )?
        .into())
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
            Self::from_hex(s).map_err(de::Error::custom)
        } else {
            let slice: &[u8] = Deserialize::deserialize(deserializer)?;
            Self::from_slice(slice).map_err(de::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use borsh::{BorshDeserialize, BorshSerialize};
    use serde::{Deserialize, Serialize};

    use crate::address::Address;
    use crate::crypto::{generate_keypair, sha256, sign_prehash};

    use super::Signature;

    #[test]
    fn hex_signature() {
        let s = "0x0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101".to_string();
        let signature = Signature::from_hex(&s).unwrap();
        assert_eq!(s, signature.to_string());
        assert_eq!([1; 65], signature.to_bytes());

        // 64-bytes
        let s = "0x01010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101";
        assert!(Signature::from_hex(s).is_err());
        // 66-bytes
        let s = "0x010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101";
        assert!(Signature::from_hex(s).is_err());
        // 65-bytes, no 0x-prefix
        let s = "0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101";
        assert!(Signature::from_hex(s).is_err());
        // 65-bytes, invalid recovery id
        let s = "0x0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010105";
        assert!(Signature::from_hex(s).is_err());
    }

    #[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
    struct TestStruct {
        signature: Signature,
    }

    #[test]
    fn serde_signature() {
        let test_struct = TestStruct {
            signature: Signature::from_bytes([1; 65]).unwrap(),
        };

        let json = serde_json::to_string(&test_struct).unwrap();
        assert_eq!(
            &json,
            r#"{"signature":"0x0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"}"#
        );

        let test_struct_deserialized: TestStruct = serde_json::from_str(&json).unwrap();
        assert_eq!(test_struct, test_struct_deserialized);

        // Missing 0x-prefix should fail
        assert!(
            serde_json::from_str::<TestStruct>(
                r#"{"signature":"0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"}"#,
            ).is_err()
        );
    }

    #[test]
    fn borsh_signature() {
        let test_struct = TestStruct {
            signature: Signature::from_bytes([1; 65]).unwrap(),
        };
        assert_eq!(&borsh::to_vec(&test_struct).unwrap(), &[1; 65]);
        assert_eq!(test_struct, borsh::from_slice(&[1; 65]).unwrap());
    }

    #[test]
    fn recover() {
        let keypair = generate_keypair();
        let message = &[1, 2, 3];
        let prehash = sha256(message);

        let (signature_bytes, recid) = sign_prehash(&keypair.secret_key(), prehash);
        let signature = Signature::from_parts(signature_bytes, recid).unwrap();

        assert_eq!(
            signature.recover_from_prehash(prehash).unwrap(),
            Address::from(keypair.public_key()),
        );
    }
}
