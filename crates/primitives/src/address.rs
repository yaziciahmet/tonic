use std::fmt::Display;

use serde::{Deserialize, Serialize};
use tonic_crypto_utils::{keccak256::keccak256, secp256k1::PublicKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Address([u8; 20]);

impl Address {
    #[cfg(feature = "test-helpers")]
    pub fn random() -> Self {
        Self(rand::random())
    }

    /// Try converting 0x-prefixed hex string to `Address`
    pub fn try_from_str(s: &str) -> Result<Self, AddressConversionError> {
        if !s.starts_with("0x") {
            return Err(AddressConversionError::Missing0xPrefix);
        }

        let mut arr = [0u8; 20];
        hex::decode_to_slice(&s[2..], &mut arr)?;

        Ok(Self(arr))
    }

    pub fn try_from_slice(slice: &[u8]) -> Result<Self, AddressConversionError> {
        Ok(Self(
            slice
                .try_into()
                .map_err(|_| AddressConversionError::TryFromSliceError)?,
        ))
    }

    pub fn from_array(arr: [u8; 20]) -> Self {
        Self(arr)
    }

    /// Convert the address to 0x-prefixed hex string
    pub fn to_string(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn to_array(&self) -> [u8; 20] {
        self.0
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl From<PublicKey> for Address {
    fn from(public: PublicKey) -> Self {
        let hash = keccak256(&public.serialize_uncompressed()[1..]);
        Address::from_array(hash[12..].try_into().expect("Exactly 20 bytes array"))
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum AddressConversionError {
    #[error("missing 0x prefix")]
    Missing0xPrefix,
    #[error("{0}")]
    FromHexError(#[from] hex::FromHexError),
    #[error("could not convert slice to address")]
    TryFromSliceError,
}

#[cfg(test)]
mod tests {
    use tonic_crypto_utils::secp256k1::PublicKey;

    use super::Address;

    const ADDRESS_HEX_AND_ARRAY: &[(&str, [u8; 20])] = &[
        (
            "0x000102030405060708090a0b0c0d0e0f10111213",
            [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
            ],
        ),
        (
            "0xffffffffffffffffffffffffffffffffffffffff",
            [
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255, 255, 255, 255,
            ],
        ),
    ];

    #[test]
    fn hex_address_conversion() {
        for (s, arr) in ADDRESS_HEX_AND_ARRAY {
            let addr = Address::try_from_str(s).unwrap();
            assert_eq!(addr, Address(*arr));

            let addr_str = addr.to_string();
            assert_eq!(addr_str, *s);
        }
    }

    #[test]
    fn slice_address_conversion() {
        for (s, arr) in ADDRESS_HEX_AND_ARRAY {
            let addr = Address::try_from_slice(arr).unwrap();
            assert_eq!(addr, Address(*arr));

            let addr_str = addr.to_string();
            assert_eq!(addr_str, *s);
        }
    }

    #[test]
    fn from_secp256k1_public_key() {
        let public = PublicKey::from_byte_array_uncompressed(&[
            4, 132, 191, 117, 98, 38, 43, 189, 105, 64, 8, 87, 72, 243, 190, 106, 250, 82, 174, 49,
            113, 85, 24, 30, 206, 49, 182, 99, 81, 204, 255, 164, 176, 140, 196, 61, 99, 178, 133,
            157, 70, 159, 238, 21, 243, 28, 158, 219, 83, 36, 38, 110, 111, 208, 64, 126, 135, 56,
            45, 96, 252, 69, 17, 172, 216,
        ])
        .unwrap();
        let address = Address::from(public);
        assert_eq!(
            address,
            Address::try_from_str("0x6370eF2f4Db3611D657b90667De398a2Cc2a370C").unwrap()
        );
    }
}
