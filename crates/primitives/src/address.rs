use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    use super::{Address, AddressConversionError};

    const ADDRESS_HEX_AND_ARRAY: &[(&str, [u8; 20])] = &[(
        "0x000102030405060708090a0b0c0d0e0f10111213",
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
        ],
    )];

    #[test]
    fn hex_address_conversion() {
        for (s, arr) in ADDRESS_HEX_AND_ARRAY {
            let addr = Address::try_from_str(s).unwrap();
            assert_eq!(addr, Address(*arr));

            let addr_str = addr.to_string();
            assert_eq!(addr_str, s.to_string());
        }

        let invalid_data: Vec<(&str, AddressConversionError)> = vec![
            ("", AddressConversionError::Missing0xPrefix),
            (
                "0x",
                AddressConversionError::FromHexError(hex::FromHexError::InvalidStringLength),
            ),
            // 19 bytes
            (
                "0x000102030405060708090a0b0c0d0e0f101112",
                AddressConversionError::FromHexError(hex::FromHexError::InvalidStringLength),
            ),
            // 21 bytes
            (
                "0x000102030405060708090a0b0c0d0e0f1011121314",
                AddressConversionError::FromHexError(hex::FromHexError::InvalidStringLength),
            ),
            // No 0x-prefix
            (
                "000102030405060708090a0b0c0d0e0f10111213",
                AddressConversionError::Missing0xPrefix,
            ),
        ];
        for (s, expected_err) in invalid_data {
            let err = Address::try_from_str(s).unwrap_err();
            assert_eq!(err, expected_err);
        }
    }

    #[test]
    fn slice_address_conversion() {
        for (_, arr) in ADDRESS_HEX_AND_ARRAY {
            let addr = Address::try_from_slice(arr).unwrap();
            assert_eq!(addr, Address(*arr));
        }

        let invalid_data: Vec<&[u8]> = vec![
            &[],
            // 19 bytes
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
            ],
            // 21 bytes
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
            ],
        ];
        for slice in invalid_data {
            let err = Address::try_from_slice(slice).unwrap_err();
            assert_eq!(err, AddressConversionError::TryFromSliceError);
        }
    }
}
