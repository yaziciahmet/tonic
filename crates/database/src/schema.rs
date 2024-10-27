use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub type SchemaName = &'static str;

pub trait Schema {
    const NAME: SchemaName;
    type Key: Serialize + DeserializeOwned + Debug;
    type Value: Serialize + DeserializeOwned + Debug;
}

#[macro_export]
macro_rules! define_schema {
    ($(#[$doc:meta])* ($name:ident) $key:ty => $value:ty) => {
        $(#[$doc])*
        #[derive(Debug)]
        pub struct $name;

        impl $crate::schema::Schema for $name {
            const NAME: $crate::schema::SchemaName = stringify!($name);
            type Key = $key;
            type Value = $value;
        }
    };
}

#[cfg(feature = "test-helpers")]
crate::define_schema!(
    /// A very very dummy schema
    (Dummy) u64 => u64
);

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TestBlock {
        height: u64,
        hash: [u8; 32],
        data: Vec<u8>,
    }

    // Verify that macro resolves without error
    crate::define_schema!(
        /// Block by height
        (TestBlocks) u64 => TestBlock
    );
    crate::define_schema!(
        /// Last block hash
        (LastBlockHash) () => [u8; 32]
    );
    crate::define_schema!(
        /// Processed transactions
        (ProcessedTransactions) [u8; 32] => ()
    );
}
