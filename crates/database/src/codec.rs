use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn serialize<T: Serialize>(item: &T) -> Vec<u8> {
    bincode::serialize(item).expect("DB serialization can not fail")
}

pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> T {
    bincode::deserialize(bytes).expect("DB deserialization can not fail")
}
