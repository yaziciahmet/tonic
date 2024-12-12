use anyhow::anyhow;
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Clone)]
pub struct GossipCodec {
    max_message_size: usize,
}

impl GossipCodec {
    pub fn new(max_message_size: usize) -> Self {
        Self { max_message_size }
    }

    pub fn serialize<T: BorshSerialize>(&self, value: &T) -> anyhow::Result<Vec<u8>> {
        let data = borsh::to_vec(value).map_err(|e| anyhow!("{e}"))?;

        self.verify_message_size(data.len())?;

        Ok(data)
    }

    pub fn deserialize<T: BorshDeserialize>(&self, data: &[u8]) -> anyhow::Result<T> {
        self.verify_message_size(data.len())?;

        borsh::from_slice(data).map_err(|e| anyhow!("{e}"))
    }

    fn verify_message_size(&self, size: usize) -> anyhow::Result<()> {
        if size > self.max_message_size {
            Err(anyhow!(
                "P2P message size exceeds maximum limit. size={} limit={}",
                size,
                self.max_message_size
            ))
        } else {
            Ok(())
        }
    }
}
