use tiny_keccak::{Hasher, Keccak};

pub fn keccak256(msg: &[u8]) -> [u8; 32] {
    let mut keccak = Keccak::v256();
    keccak.update(msg);
    let mut hash = [0u8; 32];
    keccak.finalize(&mut hash);
    hash
}

#[cfg(test)]
mod tests {
    use super::keccak256;

    #[test]
    fn test_keccak256() {
        let msg = &[1, 2, 3];
        assert_eq!(
            keccak256(msg),
            [
                241, 136, 94, 218, 84, 183, 160, 83, 49, 140, 212, 30, 32, 147, 34, 13, 171, 21,
                214, 83, 129, 177, 21, 122, 54, 51, 168, 59, 253, 92, 146, 57
            ]
        );
    }
}
