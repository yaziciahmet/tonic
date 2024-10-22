use std::{array::TryFromSliceError, fmt::Display};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Address([u8; 20]);

impl Address {
    #[cfg(feature = "test-helpers")]
    pub fn random() -> Self {
        Self(rand::random())
    }

    pub fn try_from_str(s: &str) -> Result<Self, hex::FromHexError> {
        let s = s.trim_start_matches("0x");

        let mut arr = [0u8; 20];
        hex::decode_to_slice(s, &mut arr)?;

        Ok(Self(arr))
    }

    pub fn try_from_slice(slice: &[u8]) -> Result<Self, TryFromSliceError> {
        slice.try_into().map(|arr| Self(arr))
    }

    pub fn from_array(arr: [u8; 20]) -> Self {
        Self(arr)
    }

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

#[cfg(test)]
mod tests {
    use super::Address;

    fn get_valid_address_data() -> Vec<(&'static str, [u8; 20])> {
        vec![
            (
                "000102030405060708090a0b0c0d0e0f10111213",
                [
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
                ],
            ),
            (
                "0x000102030405060708090a0b0c0d0e0f10111213",
                [
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
                ],
            ),
        ]
    }

    fn get_invalid_address_data() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("", &[]),
            ("0x", &[]),
            // 19 bytes
            (
                "000102030405060708090a0b0c0d0e0f101112",
                &[
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                ],
            ),
            (
                "0x000102030405060708090a0b0c0d0e0f101112",
                &[
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                ],
            ),
            // 21 bytes
            (
                "000102030405060708090a0b0c0d0e0f1011121314",
                &[
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                ],
            ),
            (
                "0x000102030405060708090a0b0c0d0e0f1011121314",
                &[
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                ],
            ),
        ]
    }

    #[test]
    fn hex_address_conversion() {
        for (s, arr) in get_valid_address_data() {
            let addr = Address::try_from_str(s).unwrap();
            assert_eq!(addr, Address(arr));

            let addr_str = addr.to_string();
            assert!(addr_str == s || addr_str == format!("0x{}", s));
        }

        for (s, _) in get_invalid_address_data() {
            assert!(Address::try_from_str(s).is_err());
        }
    }

    #[test]
    fn slice_address_conversion() {
        for (_, arr) in get_valid_address_data() {
            let addr = Address::try_from_slice(&arr).unwrap();
            assert_eq!(addr, Address(arr));
        }

        for (_, slice) in get_invalid_address_data() {
            assert!(Address::try_from_slice(slice).is_err());
        }
    }
}
