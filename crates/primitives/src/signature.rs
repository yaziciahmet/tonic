use std::fmt::Display;

use tonic_crypto_utils::secp256k1;

use crate::address::Address;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Signature([u8; 65]);

impl Signature {
    pub fn try_from_array(arr: [u8; 65]) -> Result<Self, SignatureError> {
        Self::check_v(arr[64])?;
        Ok(Self(arr))
    }

    pub fn try_from_slice(slice: &[u8]) -> Result<Self, SignatureError> {
        if slice.len() != 65 {
            return Err(SignatureError::TryFromSliceError);
        }

        Self::check_v(slice[64])?;
        Ok(Self(
            slice
                .try_into()
                .expect("65 sized slice to array can not fail"),
        ))
    }

    pub fn try_from_str(s: &str) -> Result<Self, SignatureError> {
        if !s.starts_with("0x") {
            return Err(SignatureError::Missing0xPrefix);
        }

        let mut arr = [0u8; 65];
        hex::decode_to_slice(&s[2..], &mut arr)?;

        Self::check_v(arr[64])?;
        Ok(Self(arr))
    }

    pub fn try_from_rsv(r: &[u8; 32], s: &[u8; 32], v: u8) -> Result<Self, SignatureError> {
        Self::check_v(v)?;

        let mut full = [0u8; 65];
        full[0..32].copy_from_slice(r);
        full[32..64].copy_from_slice(s);
        full[64] = v;

        Ok(Self(full))
    }

    pub fn to_array(&self) -> [u8; 65] {
        self.0
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn r(&self) -> &[u8; 32] {
        self.0[0..32].try_into().expect("Has exactly 32 bytes")
    }

    pub fn s(&self) -> &[u8; 32] {
        self.0[32..64].try_into().expect("Has exactly 32 bytes")
    }

    pub fn v(&self) -> u8 {
        self.0[64]
    }

    pub fn recover_signer(&self, msg: &[u8; 32]) -> Result<Address, SignatureError> {
        let public = secp256k1::recover_ecdsa(&self.0, msg)?;
        Ok(public.into())
    }

    fn check_v(v: u8) -> Result<(), SignatureError> {
        if v <= 3 {
            Ok(())
        } else {
            Err(SignatureError::Secp256k1Error(
                secp256k1::Error::InvalidRecoveryId,
            ))
        }
    }
}

impl Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum SignatureError {
    #[error("missing 0x prefix")]
    Missing0xPrefix,
    #[error("{0}")]
    FromHexError(#[from] hex::FromHexError),
    #[error("could not convert slice to signature")]
    TryFromSliceError,
    #[error("{0}")]
    Secp256k1Error(#[from] secp256k1::Error),
}

#[cfg(feature = "test-helpers")]
#[cfg(test)]
mod tests {
    use crate::address::Address;

    use super::Signature;

    const SIGNATURE_HEX_AND_ARRAY: &[(&str, [u8; 65])] = &[
        (
            "0x4447f3ae22e786de332cc5ed3557ebe66ee0c901b02a5353fdcd41131c4e9881c030cc7ebf27f7ec0249c6d00ca67a19f03a908519e5faa764eb8de0d7fbc33201",
            [68, 71, 243, 174, 34, 231, 134, 222, 51, 44, 197, 237, 53, 87, 235, 230, 110, 224, 201, 1, 176, 42, 83, 83, 253, 205, 65, 19, 28, 78, 152, 129, 192, 48, 204, 126, 191, 39, 247, 236, 2, 73, 198, 208, 12, 166, 122, 25, 240, 58, 144, 133, 25, 229, 250, 167, 100, 235, 141, 224, 215, 251, 195, 50, 1],
        ),
        (
            "0x57bccaf7957537d45990fdba05bbc8f9a0638366bb35c62dee2d63cb33736ce600882d92af0c3f3f4234660415cafb2f9438047c7bb9d193df01bde522164faf01",
            [87, 188, 202, 247, 149, 117, 55, 212, 89, 144, 253, 186, 5, 187, 200, 249, 160, 99, 131, 102, 187, 53, 198, 45, 238, 45, 99, 203, 51, 115, 108, 230, 0, 136, 45, 146, 175, 12, 63, 63, 66, 52, 102, 4, 21, 202, 251, 47, 148, 56, 4, 124, 123, 185, 209, 147, 223, 1, 189, 229, 34, 22, 79, 175, 1],
        ),
    ];

    #[test]
    fn hex_signature_conversion() {
        for (s, arr) in SIGNATURE_HEX_AND_ARRAY {
            let sig = Signature::try_from_str(s).unwrap();
            assert_eq!(sig, Signature(*arr));

            let sig_str = sig.to_string();
            assert_eq!(sig_str, *s);
        }
    }

    #[test]
    fn slice_signature_conversion() {
        for (s, arr) in SIGNATURE_HEX_AND_ARRAY {
            let sig = Signature::try_from_slice(arr).unwrap();
            assert_eq!(sig, Signature(*arr));

            let sig_str = sig.to_string();
            assert_eq!(sig_str, *s);
        }
    }

    #[test]
    fn rsv_signature_conversion() {
        for (_, arr) in SIGNATURE_HEX_AND_ARRAY {
            let r = &arr[0..32].try_into().unwrap();
            let s = &arr[32..64].try_into().unwrap();
            let v = arr[64];

            let sig = Signature::try_from_rsv(r, s, v).unwrap();
            assert_eq!(sig, Signature(*arr));
            assert_eq!(sig.r(), r);
            assert_eq!(sig.s(), s);
            assert_eq!(sig.v(), v);
        }
    }

    #[test]
    fn recover_signer() {
        let address = Address::try_from_str("0x6370eF2f4Db3611D657b90667De398a2Cc2a370C").unwrap();
        let sig = Signature::try_from_str("0x4447f3ae22e786de332cc5ed3557ebe66ee0c901b02a5353fdcd41131c4e98814030cc7ebf27f7ec0249c6d00ca67a19f03a908519e5faa764eb8de0d7fbc33201").unwrap();
        let hash = b"11111111111111111111111111111111";

        assert_eq!(sig.recover_signer(hash).unwrap(), address);
    }
}
