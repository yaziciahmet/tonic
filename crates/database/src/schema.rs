use std::fmt::Debug;

use borsh::{BorshDeserialize, BorshSerialize};

pub type SchemaName = &'static str;

/// Schema trait wraps a simple key-value schema info
/// for easy serialization of the key and values.
pub trait Schema {
    const NAME: SchemaName;
    type PrefixKey: BorshSerialize + BorshDeserialize + Debug;
    type Key: BorshSerialize + BorshDeserialize + Debug;
    type Value: BorshSerialize + BorshDeserialize + Debug;
}

/// Macro to easily define a key-value schema by implementing `Schema` trait.
#[macro_export]
macro_rules! define_schema {
    ($(#[$doc:meta])* ($name:ident) $key:ty => $value:ty) => {
        $(#[$doc])*
        #[derive(Debug)]
        pub struct $name;

        impl $crate::schema::Schema for $name {
            const NAME: $crate::schema::SchemaName = stringify!($name);
            type PrefixKey = ();
            type Key = $key;
            type Value = $value;
        }
    };
    ($(#[$doc:meta])* ($name:ident) $prefix_key:ty => $key:ty => $value:ty) => {
        $(#[$doc])*
        #[derive(Debug)]
        pub struct $name;

        impl $crate::schema::Schema for $name {
            const NAME: $crate::schema::SchemaName = stringify!($name);
            type PrefixKey = $prefix_key;
            type Key = $key;
            type Value = $value;
        }
    };
}

#[cfg(test)]
mod tests {
    use borsh::{BorshDeserialize, BorshSerialize};

    #[derive(Debug, BorshSerialize, BorshDeserialize)]
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
    crate::define_schema!(
        /// Double key schema
        (DoubleKeySchema) u64 => u64 => u128
    );
}
