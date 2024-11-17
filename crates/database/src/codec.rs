use borsh::{BorshDeserialize, BorshSerialize};

pub fn serialize<T: BorshSerialize>(item: &T) -> Vec<u8> {
    borsh::to_vec(item).expect("DB serialization can not fail")
}

pub fn deserialize<T: BorshDeserialize>(bytes: &[u8]) -> T {
    borsh::from_slice(bytes).expect("DB deserialization can not fail")
}
