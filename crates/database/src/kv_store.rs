use std::collections::{BTreeMap, HashMap};

use crate::schema::{Schema, SchemaName};

/// Database trait to implement to provide key-value access capabilities.
pub trait KeyValueAccessor {
    /// Get a value from the schema by key.
    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, rocksdb::Error>;

    /// Get multiple values from the schema by list of keys.
    /// When using this method, ensure that keys are already sorted
    /// to avoid unexpected behaviour.
    fn multi_get<S: Schema>(
        &self,
        keys: impl IntoIterator<Item = S::Key>,
    ) -> Result<Vec<Option<S::Value>>, rocksdb::Error>;

    /// Check if a key exists in the schema
    fn exists<S: Schema>(&self, key: &S::Key) -> Result<bool, rocksdb::Error>;
}

/// Database trait to implement to provide key-value mutation capabilities.
pub trait KeyValueMutator {
    /// Put a key-value pair into the column
    fn put<S: Schema>(&mut self, key: &S::Key, value: &S::Value) -> Result<(), rocksdb::Error>;

    /// Delete a key from the column
    fn delete<S: Schema>(&mut self, key: &S::Key) -> Result<(), rocksdb::Error>;
}

/// `IteratorMode` is a `column` wrapped `rocksdb::IteratorMode`.
#[derive(Debug)]
pub enum IteratorMode<'b, S: Schema> {
    Start,
    End,
    Forward(&'b S::Key),
    Reverse(&'b S::Key),
}

/// Database trait to implement to provide key-value iteration capabilites.
pub trait KeyValueIterator {
    fn iterator<'a, S: Schema>(
        &'a self,
        mode: IteratorMode<'a, S>,
    ) -> impl Iterator<Item = Result<(S::Key, S::Value), rocksdb::Error>> + 'a;
}

/// Database trait to implement committing the unsaved changes.
pub trait Commitable {
    fn commit(self) -> Result<(), rocksdb::Error>;
}

/// Struct representing a single write operation.
pub enum WriteOperation {
    Put(Vec<u8>),
    Delete,
}

pub type Changes = HashMap<SchemaName, BTreeMap<Vec<u8>, WriteOperation>>;
