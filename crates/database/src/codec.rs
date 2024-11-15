use borsh::{BorshDeserialize, BorshSerialize};

pub fn serialize<T: BorshSerialize>(item: &T) -> Vec<u8> {
    tonic_codec::serialize(item).expect("DB serialization can not fail")
}

pub fn deserialize<T: BorshDeserialize>(bytes: &[u8]) -> T {
    tonic_codec::deserialize(bytes).expect("DB deserialization can not fail")
}
