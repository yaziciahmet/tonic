use std::collections::HashMap;

use crate::codec;
use crate::kv_store::{
    Changes, Commitable, KeyValueAccessor, KeyValueIterator, KeyValueMutator, WriteOperation,
};
use crate::rocksdb::{FullAccess, RocksDB};
use crate::schema::{Schema, SchemaName};

/// `InMemoryTransaction` collects the transaction operations
/// in memory, and on commit, batches all the changes at once.
pub struct InMemoryTransaction<'a> {
    db: &'a RocksDB<FullAccess>,
    changes: Changes,
}

impl<'a> InMemoryTransaction<'a> {
    pub fn new(db: &'a RocksDB<FullAccess>) -> Self {
        Self {
            db,
            changes: HashMap::new(),
        }
    }

    fn get_from_changes(&self, schema: SchemaName, key: &Vec<u8>) -> Option<&WriteOperation> {
        self.changes.get(schema).and_then(|btree| btree.get(key))
    }
}

impl<'a> KeyValueAccessor for InMemoryTransaction<'a> {
    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        if let Some(operation) = self.get_from_changes(S::NAME, &key_bytes) {
            match operation {
                WriteOperation::Put(value_bytes) => Ok(Some(codec::deserialize(value_bytes))),
                WriteOperation::Delete => Ok(None),
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
                        WriteOperation::Put(value_bytes) => {
                            Some(codec::deserialize(value_bytes.as_slice()))
                        }
                        WriteOperation::Delete => None,
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
                WriteOperation::Put(_) => Ok(true),
                WriteOperation::Delete => Ok(false),
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
            .insert(key_bytes, WriteOperation::Put(value_bytes));
        Ok(())
    }

    fn delete<S: Schema>(&mut self, key: &S::Key) -> Result<(), rocksdb::Error> {
        let key_bytes = codec::serialize(key);

        self.changes
            .entry(S::NAME)
            .or_default()
            .insert(key_bytes, WriteOperation::Delete);
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

impl<'a> Commitable for InMemoryTransaction<'a> {
    fn commit(self) -> Result<(), rocksdb::Error> {
        self.db.commit_changes(self.changes)
    }
}

#[cfg(test)]
mod tests {
    use crate::kv_store::{Commitable, KeyValueAccessor, KeyValueMutator};
    use crate::rocksdb::create_test_db;
    use crate::schema::Dummy;

    #[test]
    fn put() {
        let db = create_test_db();
        let mut tx = db.transaction();

        tx.put::<Dummy>(&1, &100).unwrap();
        assert_eq!(tx.get::<Dummy>(&1).unwrap(), Some(100));
        assert_eq!(db.get::<Dummy>(&1).unwrap(), None);

        tx.commit().unwrap();
        assert_eq!(db.get::<Dummy>(&1).unwrap(), Some(100));
    }

    #[test]
    fn put_and_delete() {
        let db = create_test_db();
        let mut tx = db.transaction();

        tx.put::<Dummy>(&1, &100).unwrap();

        tx.delete::<Dummy>(&1).unwrap();
        assert_eq!(tx.get::<Dummy>(&1).unwrap(), None);

        tx.commit().unwrap();
        assert_eq!(db.get::<Dummy>(&1).unwrap(), None);
    }

    #[test]
    fn get_prev_data() {
        let mut db = create_test_db();
        db.put::<Dummy>(&1, &100).unwrap();

        let tx = db.transaction();
        assert_eq!(tx.get::<Dummy>(&1).unwrap(), Some(100));
    }

    #[test]
    fn put_and_multi_get() {
        let mut db = create_test_db();
        db.put::<Dummy>(&1, &100).unwrap();

        let mut tx = db.transaction();

        tx.put::<Dummy>(&2, &200).unwrap();
        tx.put::<Dummy>(&3, &300).unwrap();
        tx.put::<Dummy>(&4, &400).unwrap();

        assert_eq!(
            tx.multi_get::<Dummy>(vec![1, 2, 4, 5]).unwrap(),
            vec![Some(100), Some(200), Some(400), None]
        );

        tx.commit().unwrap();
        assert_eq!(
            db.multi_get::<Dummy>(vec![1, 2, 4, 5]).unwrap(),
            vec![Some(100), Some(200), Some(400), None]
        );
    }
}
