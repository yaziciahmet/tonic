#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Signature([u8; 65]);

impl Signature {
    pub fn from_array(arr: [u8; 65]) -> Self {
        Self(arr)
    }

    pub fn try_from_slice(slice: &[u8]) -> Result<Self, SignatureConversionError> {
        Ok(Self(slice.try_into().map_err(|_| {
            SignatureConversionError::TryFromSliceError
        })?))
    }

    pub fn try_from_str(s: &str) -> Result<Self, SignatureConversionError> {
        if !s.starts_with("0x") {
            return Err(SignatureConversionError::Missing0xPrefix);
        }

        let mut arr = [0u8; 65];
        hex::decode_to_slice(&s[2..], &mut arr)?;

        Ok(Self(arr))
    }

    pub fn from_rsv(r: &[u8; 32], s: &[u8; 32], v: u8) -> Self {
        let mut full = [0u8; 65];
        full[0..32].copy_from_slice(r);
        full[32..64].copy_from_slice(s);
        full[64] = v;

        Self(full)
    }

    pub fn to_array(&self) -> [u8; 65] {
        self.0
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn to_string(&self) -> String {
        format!("0x{}", hex::encode(self.0))
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
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum SignatureConversionError {
    #[error("missing 0x prefix")]
    Missing0xPrefix,
    #[error("{0}")]
    FromHexError(#[from] hex::FromHexError),
    #[error("could not convert slice to address")]
    TryFromSliceError,
}

#[cfg(test)]
mod tests {
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

            let sig = Signature::from_rsv(r, s, v);
            assert_eq!(sig, Signature(*arr));
            assert_eq!(sig.r(), r);
            assert_eq!(sig.s(), s);
            assert_eq!(sig.v(), v);
        }
    }
}
