use crate::schema::Schema;

pub trait KeyValueAccessor {
    /// Get a value from the column by key
    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, rocksdb::Error>;

    /// Get multiple values from the column by list of keys.
    /// When using this method, ensure that keys are already sorted
    /// to avoid unexpected behaviour.
    fn multi_get<S: Schema>(
        &self,
        keys: impl IntoIterator<Item = S::Key>,
    ) -> Result<Vec<Option<S::Value>>, rocksdb::Error>;

    /// Check if a key exists in the column
    fn exists<S: Schema>(&self, key: &S::Key) -> Result<bool, rocksdb::Error>;
}

/// Database trait to implement to have mutator access
pub trait KeyValueMutator {
    /// Put a key-value pair into the column
    fn put<S: Schema>(&mut self, key: &S::Key, value: &S::Value) -> Result<(), rocksdb::Error>;

    /// Delete a key from the column
    fn delete<S: Schema>(&mut self, key: &S::Key) -> Result<(), rocksdb::Error>;
}

pub trait KeyValueIterator {
    fn iterator<'a, S: Schema>(
        &'a self,
        mode: IteratorMode<'a, S>,
    ) -> impl Iterator<Item = Result<(S::Key, S::Value), rocksdb::Error>> + 'a;
}

/// `IteratorMode` is a `column` wrapped `rocksdb::IteratorMode`.
#[derive(Debug)]
pub enum IteratorMode<'b, S: Schema> {
    Start,
    End,
    Forward(&'b S::Key),
    Reverse(&'b S::Key),
}
