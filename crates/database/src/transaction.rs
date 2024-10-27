use std::collections::{BTreeMap, HashMap};

use crate::codec;
use crate::kv_store::{KeyValueAccessor, KeyValueIterator, KeyValueMutator};
use crate::rocksdb::{FullAccess, RocksDB};
use crate::schema::{Schema, SchemaName};

pub enum TxOperation {
    Put(Vec<u8>),
    Delete,
}

pub struct InMemoryTransaction<'a> {
    db: &'a RocksDB<FullAccess>,
    changes: HashMap<SchemaName, BTreeMap<Vec<u8>, TxOperation>>,
}

impl<'a> InMemoryTransaction<'a> {
    pub fn new(db: &'a RocksDB<FullAccess>) -> Self {
        Self {
            db,
            changes: HashMap::new(),
        }
    }
    fn get_from_changes(&self, schema: SchemaName, key: &Vec<u8>) -> Option<&TxOperation> {
        self.changes.get(schema).and_then(|btree| btree.get(key))
    }
}

impl<'a> KeyValueAccessor for InMemoryTransaction<'a> {
    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        if let Some(operation) = self.get_from_changes(S::NAME, &key_bytes) {
            match operation {
                TxOperation::Put(value_bytes) => Ok(Some(codec::deserialize(value_bytes))),
                TxOperation::Delete => Ok(None),
            }
        } else {
            Ok(self
                .db
                .raw_get(S::NAME, &key_bytes)?
                .map(|bytes| codec::deserialize(bytes.as_ref())))
        }
    }

    fn multi_get<S: Schema>(
        &self,
        keys: impl IntoIterator<Item = S::Key>,
    ) -> Result<Vec<Option<S::Value>>, rocksdb::Error> {
        if let Some(btree) = self.changes.get(S::NAME) {
            let mut values: Vec<Option<S::Value>> = vec![];
            for key in keys.into_iter() {
                let key_bytes = codec::serialize(&key);

                let value: Option<S::Value> = if let Some(operation) = btree.get(&key_bytes) {
                    match operation {
                        TxOperation::Put(value_bytes) => {
                            Some(codec::deserialize(value_bytes.as_slice()))
                        }
                        TxOperation::Delete => None,
                    }
                } else {
                    self.db
                        .raw_get(S::NAME, &key_bytes)?
                        .map(|bytes| codec::deserialize(bytes.as_ref()))
                };

                values.push(value);
            }

            Ok(values)
        } else {
            self.db.multi_get::<S>(keys)
        }
    }

    fn exists<S: Schema>(&self, key: &S::Key) -> Result<bool, rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        if let Some(operation) = self.get_from_changes(S::NAME, &key_bytes) {
            match operation {
                TxOperation::Put(_) => Ok(true),
                TxOperation::Delete => Ok(false),
            }
        } else {
            Ok(self.db.raw_get(S::NAME, &key_bytes)?.is_some())
        }
    }
}

impl<'a> KeyValueMutator for InMemoryTransaction<'a> {
    fn put<S: Schema>(&mut self, key: &S::Key, value: &S::Value) -> Result<(), rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        let value_bytes = codec::serialize(value);

        self.changes
            .entry(S::NAME)
            .or_default()
            .insert(key_bytes, TxOperation::Put(value_bytes));
        Ok(())
    }

    fn delete<S: Schema>(&mut self, key: &S::Key) -> Result<(), rocksdb::Error> {
        let key_bytes = codec::serialize(key);

        self.changes
            .entry(S::NAME)
            .or_default()
            .insert(key_bytes, TxOperation::Delete);
        Ok(())
    }
}

impl<'a> KeyValueIterator for InMemoryTransaction<'a> {
    fn iterator<'b, S: Schema>(
        &'b self,
        mode: crate::kv_store::IteratorMode<'b, S>,
    ) -> impl Iterator<Item = Result<(S::Key, S::Value), rocksdb::Error>> + 'b {
        // Remove and see what happens :)
        if false {
            return self.db.iterator(mode);
        }
        unimplemented!("Iterators not implemented for transactions. Find another way.");
    }
}
