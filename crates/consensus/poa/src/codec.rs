use borsh::{BorshDeserialize, BorshSerialize};

pub fn serialize<T: BorshSerialize>(item: &T) -> Vec<u8> {
    borsh::to_vec(item).expect("Consensus serialization can not fail")
}

pub fn deserialize<T: BorshDeserialize>(bytes: &[u8]) -> T {
    borsh::from_slice(bytes).expect("Consensus deserialization can not fail")
}

pub fn serialize_to<T: BorshSerialize>(item: &T, buf: &mut [u8]) {
    let mut buf = buf;
    item.serialize(&mut buf)
        .expect("Consensus serialization into buffer failed");
}
