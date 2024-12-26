use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use rocksdb::{
    BlockBasedOptions, Cache, ColumnFamily, ColumnFamilyDescriptor, DBCompressionType,
    DBPinnableSlice, Options, ReadOptions, SliceTransform, SnapshotWithThreadMode, WriteBatch, DB,
};

use crate::codec;
use crate::config::Config;
use crate::kv_store::{
    Changes, IteratorMode, KeyValueAccessor, KeyValueIterator, KeyValueMutator, Snapshottable,
    Transactional, WriteOperation,
};
use crate::schema::{Schema, SchemaMetadata, SchemaName};
use crate::transaction::InMemoryTransaction;

/// Helper traits for enabling view-only access to database
trait ViewAccess {}
trait MutatorAccess {}

pub struct FullAccess;
impl ViewAccess for FullAccess {}
impl MutatorAccess for FullAccess {}

pub struct ViewOnlyAccess;
impl ViewAccess for ViewOnlyAccess {}

/// `RocksDB` is a wrapper around `rocksdb::DB`.
/// Supports snapshots with view-only access.
pub struct RocksDB<Access> {
    inner: Arc<DB>,
    snapshot: Option<SnapshotWithThreadMode<'static, DB>>,
    read_opts: ReadOptions,
    phantom: PhantomData<Access>,
}

impl<Access> Drop for RocksDB<Access> {
    fn drop(&mut self) {
        // Drop snapshot before RocksDB
        self.snapshot = None;
    }
}

impl<Access> RocksDB<Access> {
    pub(crate) fn raw_get(
        &self,
        schema: SchemaName,
        key: &[u8],
    ) -> Result<Option<DBPinnableSlice>, rocksdb::Error> {
        let cf = self.cf_handle(schema);

        self.inner.get_pinned_cf_opt(cf, key, &self.read_opts)
    }

    pub(crate) fn raw_multi_get<'a>(
        &self,
        schema: SchemaName,
        keys: impl IntoIterator<Item = &'a [u8]>,
    ) -> Vec<Result<Option<DBPinnableSlice>, rocksdb::Error>> {
        let cf = self.cf_handle(schema);

        self.inner
            .batched_multi_get_cf_opt(cf, keys, true, &self.read_opts)
    }

    pub(crate) fn raw_iterator<'a>(
        &'a self,
        schema: SchemaName,
        rocks_db_mode: rocksdb::IteratorMode,
        opts: ReadOptions,
    ) -> impl Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>> + 'a {
        let cf = self.cf_handle(schema);

        self.inner.iterator_cf_opt(cf, opts, rocks_db_mode)
    }

    pub(crate) fn raw_put(
        &mut self,
        schema: SchemaName,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), rocksdb::Error> {
        let cf = self.cf_handle(schema);

        self.inner.put_cf(cf, key, value)
    }

    pub(crate) fn raw_delete(
        &mut self,
        schema: SchemaName,
        key: &[u8],
    ) -> Result<(), rocksdb::Error> {
        let cf = self.cf_handle(schema);

        self.inner.delete_cf(cf, key)
    }

    pub(crate) fn raw_write_batch(&self, changes: Changes) -> Result<(), rocksdb::Error> {
        let mut batch = WriteBatch::default();
        for (schema, btree) in changes {
            let cf = self.cf_handle(schema);
            for (key, operation) in btree {
                match operation {
                    WriteOperation::Put(value) => batch.put_cf(cf, key, value),
                    WriteOperation::Delete => batch.delete_cf(cf, key),
                }
            }
        }

        self.inner.write(batch)
    }

    /// Asserts existence of column family and returns it.
    fn cf_handle(&self, schema: SchemaName) -> &ColumnFamily {
        self.inner
            .cf_handle(schema)
            .unwrap_or_else(|| panic!("Received non-existing schema `{schema}`"))
    }

    fn read_opts(&self) -> ReadOptions {
        Self::generate_read_opts(&self.snapshot)
    }

    fn generate_read_opts(snapshot: &Option<SnapshotWithThreadMode<'static, DB>>) -> ReadOptions {
        let mut opts = ReadOptions::default();
        opts.set_verify_checksums(false);
        if let Some(snapshot) = snapshot {
            opts.set_snapshot(snapshot);
        }
        opts
    }
}

impl RocksDB<FullAccess> {
    #[cfg(feature = "test-helpers")]
    pub fn open_temp(config: Config, schemas: &[SchemaMetadata]) -> RocksDB<FullAccess> {
        let path = tempfile::tempdir().unwrap();
        RocksDB::open(path, config, schemas)
    }

    pub fn open(
        path: impl AsRef<Path>,
        config: Config,
        schemas: &[SchemaMetadata],
    ) -> RocksDB<FullAccess> {
        // Create main database options
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        // Suggested compression type is Lz4.
        // https://github.com/facebook/rocksdb/wiki/Compression
        opts.set_compression_type(DBCompressionType::Lz4);
        opts.increase_parallelism(2);
        opts.set_max_background_jobs(4);
        opts.set_max_open_files(config.max_open_files);
        opts.set_max_total_wal_size(config.max_total_wal_size);
        opts.set_bytes_per_sync(1024 * 1024);
        // 128 MB of row cache
        let cache = Cache::new_lru_cache(config.max_cache_size as usize);
        opts.set_row_cache(&cache);

        let mut block_opts = BlockBasedOptions::default();
        // Default block size is 4 KB, but suggested as 16 KB.
        // https://github.com/facebook/rocksdb/wiki/memory-usage-in-rocksdb
        block_opts.set_block_size(16 * 1024);
        // Bloom filter to reduce disk I/O on reads
        block_opts.set_bloom_filter(10.0, true);
        // Store index and filter blocks in cache
        block_opts.set_cache_index_and_filter_blocks(true);
        // Don't evict L0 filter/index blocks from the cache
        block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        // Reduces bloom filter memory usage.
        block_opts.set_optimize_filters_for_memory(true);
        // 128 MB of block cache
        let cache = Cache::new_lru_cache(config.max_cache_size as usize);
        block_opts.set_block_cache(&cache);

        let cfs = schemas.iter().map(|meta| {
            let mut cf_opts = Options::default();
            cf_opts.set_compression_type(DBCompressionType::Lz4);
            cf_opts.set_block_based_table_factory(&block_opts);
            if meta.prefix_size > 0 {
                cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(meta.prefix_size));
            }

            ColumnFamilyDescriptor::new(meta.name, cf_opts)
        });

        let inner = DB::open_cf_descriptors(&opts, path, cfs)
            .expect("Failed open RocksDB with cf descriptors");

        RocksDB {
            inner: Arc::new(inner),
            snapshot: None,
            read_opts: Self::generate_read_opts(&None),
            phantom: PhantomData,
        }
    }
}

impl<Access> KeyValueAccessor for RocksDB<Access>
where
    Access: ViewAccess,
{
    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        Ok(self
            .raw_get(S::METADATA.name, &key_bytes)?
            .map(|bytes| codec::deserialize(bytes.as_ref())))
    }

    fn multi_get<S: Schema>(
        &self,
        keys: impl IntoIterator<Item = S::Key>,
    ) -> Result<Vec<Option<S::Value>>, rocksdb::Error> {
        let keys_bytes = keys
            .into_iter()
            .map(|key| codec::serialize(&key))
            .collect::<Vec<_>>();

        let values_bytes = self.raw_multi_get(
            S::METADATA.name,
            keys_bytes.iter().map(|bytes| bytes.as_slice()),
        );
        let mut values = Vec::with_capacity(values_bytes.len());
        for bytes in values_bytes {
            let value = bytes?.map(|bytes| codec::deserialize(bytes.as_ref()));
            values.push(value);
        }

        Ok(values)
    }

    fn exists<S: Schema>(&self, key: &S::Key) -> Result<bool, rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        Ok(self.raw_get(S::METADATA.name, &key_bytes)?.is_some())
    }
}

impl<Access> KeyValueMutator for RocksDB<Access>
where
    Access: MutatorAccess,
{
    fn put<S: Schema>(&mut self, key: &S::Key, value: &S::Value) -> Result<(), rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        let value_bytes = codec::serialize(value);
        self.raw_put(S::METADATA.name, &key_bytes, &value_bytes)
    }

    fn delete<S: Schema>(&mut self, key: &S::Key) -> Result<(), rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        self.raw_delete(S::METADATA.name, &key_bytes)
    }

    fn write_batch(&mut self, changes: Changes) -> Result<(), rocksdb::Error> {
        self.raw_write_batch(changes)
    }
}

impl<Access> KeyValueIterator for RocksDB<Access>
where
    Access: ViewAccess,
{
    fn iterator<'a, S: Schema>(
        &'a self,
        mode: IteratorMode<'a, S>,
    ) -> impl Iterator<Item = Result<(S::Key, S::Value), rocksdb::Error>> + 'a {
        let key_bytes;
        let rocks_db_mode = match mode {
            IteratorMode::Start => rocksdb::IteratorMode::Start,
            IteratorMode::End => rocksdb::IteratorMode::End,
            IteratorMode::Forward(key) => {
                key_bytes = codec::serialize(key);
                rocksdb::IteratorMode::From(&key_bytes, rocksdb::Direction::Forward)
            }
            IteratorMode::Reverse(key) => {
                key_bytes = codec::serialize(key);
                rocksdb::IteratorMode::From(&key_bytes, rocksdb::Direction::Reverse)
            }
        };

        self.raw_iterator(S::METADATA.name, rocks_db_mode, self.read_opts())
            .map(|result| {
                result.map(|(key_bytes, value_bytes)| {
                    let key = codec::deserialize(&key_bytes);
                    let value = codec::deserialize(&value_bytes);
                    (key, value)
                })
            })
    }
}

impl<'a> Transactional<'a> for RocksDB<FullAccess> {
    type Transaction = InMemoryTransaction<'a>;

    fn transaction(&'a self) -> Self::Transaction {
        InMemoryTransaction::new(self)
    }
}

impl Snapshottable for RocksDB<FullAccess> {
    type Snapshot = RocksDB<ViewOnlyAccess>;

    fn snapshot(&self) -> Self::Snapshot {
        // Transmute snapshot to be static lifetime. Snapshot is dropped
        // safely before the RocksDB manually.
        let snapshot = unsafe {
            let snapshot = self.inner.snapshot();
            #[allow(clippy::missing_transmute_annotations)]
            std::mem::transmute(snapshot)
        };
        let snapshot = Some(snapshot);

        RocksDB {
            inner: self.inner.clone(),
            read_opts: Self::generate_read_opts(&snapshot),
            snapshot,
            phantom: PhantomData,
        }
    }
}

#[cfg(feature = "test-helpers")]
pub fn create_test_db(schemas: &[SchemaMetadata]) -> RocksDB<FullAccess> {
    let config = Config {
        max_open_files: 8,
        max_cache_size: 1024 * 1024,
        max_total_wal_size: 2 * 1024 * 1024,
    };
    RocksDB::open_temp(config, schemas)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use crate::kv_store::{
        IteratorMode, KeyValueAccessor, KeyValueIterator, KeyValueMutator, Snapshottable,
        WriteOperation,
    };
    use crate::schema::{Schema, SchemaMetadata};
    use crate::{codec, define_schema};

    use super::create_test_db;

    define_schema!(
        (Dummy) u64 => u64
    );

    const SCHEMAS: &[SchemaMetadata] = &[Dummy::METADATA];

    #[test]
    fn put_and_get() {
        let mut db = create_test_db(SCHEMAS);

        // Put a kv
        db.put::<Dummy>(&1, &100).unwrap();
        assert!(db.exists::<Dummy>(&1).unwrap());
        assert_eq!(db.get::<Dummy>(&1).unwrap(), Some(100));

        // Put another kv and assure that previously put exists as well
        db.put::<Dummy>(&2, &200).unwrap();
        assert!(db.exists::<Dummy>(&1).unwrap());
        assert_eq!(db.get::<Dummy>(&1).unwrap(), Some(100));
        assert!(db.exists::<Dummy>(&2).unwrap());
        assert_eq!(db.get::<Dummy>(&2).unwrap(), Some(200));
    }

    #[test]
    fn put_and_delete() {
        let mut db = create_test_db(SCHEMAS);

        // Put kvs
        db.put::<Dummy>(&1, &100).unwrap();
        db.put::<Dummy>(&2, &200).unwrap();
        assert!(db.exists::<Dummy>(&1).unwrap());
        assert!(db.exists::<Dummy>(&2).unwrap());

        // Delete one of them
        db.delete::<Dummy>(&1).unwrap();
        assert!(!db.exists::<Dummy>(&1).unwrap());
        assert!(db.exists::<Dummy>(&2).unwrap());

        // Delete the other one as well
        db.delete::<Dummy>(&2).unwrap();
        assert!(!db.exists::<Dummy>(&1).unwrap());
        assert!(!db.exists::<Dummy>(&2).unwrap());
    }

    #[test]
    fn put_and_multi_get() {
        let mut db = create_test_db(SCHEMAS);

        // Put kvs
        db.put::<Dummy>(&1, &100).unwrap();
        db.put::<Dummy>(&2, &200).unwrap();
        db.put::<Dummy>(&3, &300).unwrap();
        assert!(db.exists::<Dummy>(&1).unwrap());
        assert!(db.exists::<Dummy>(&2).unwrap());
        assert!(db.exists::<Dummy>(&3).unwrap());

        // Check multi_get
        assert_eq!(
            db.multi_get::<Dummy>(vec![1, 2, 3]).unwrap(),
            vec![Some(100), Some(200), Some(300)]
        );
        assert_eq!(
            db.multi_get::<Dummy>(vec![4, 5, 6]).unwrap(),
            vec![None, None, None]
        );
        assert_eq!(
            db.multi_get::<Dummy>(vec![1, 3, 5]).unwrap(),
            vec![Some(100), Some(300), None]
        );
    }

    #[test]
    fn iterator() {
        let mut db = create_test_db(SCHEMAS);

        // Put kvs
        db.put::<Dummy>(&1, &100).unwrap();
        db.put::<Dummy>(&2, &200).unwrap();
        db.put::<Dummy>(&3, &300).unwrap();
        assert!(db.exists::<Dummy>(&1).unwrap());
        assert!(db.exists::<Dummy>(&2).unwrap());
        assert!(db.exists::<Dummy>(&3).unwrap());

        // Iterate over all kvs
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Start)
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(1, 100), (2, 200), (3, 300)]);

        // Iterate over all kvs in reverse
        let kvs = db
            .iterator::<Dummy>(IteratorMode::End)
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(3, 300), (2, 200), (1, 100)]);

        // Iterate over kvs after 2
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Forward(&2))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(2, 200), (3, 300)]);

        // Iterate over kvs before 2 in reverse
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Reverse(&2))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(2, 200), (1, 100)]);

        // Iterate over kvs after 3
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Forward(&3))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(3, 300)]);

        // Iterate over kvs before 1 in reverse
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Reverse(&1))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(1, 100)]);

        // Iterate over keys after a key that doesn't exist, starts from the closest key
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Forward(&0))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(1, 100), (2, 200), (3, 300)]);

        // Iterate over keys before a key that doesn't exist in reverse, starts from the closest key
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Reverse(&4))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![(3, 300), (2, 200), (1, 100)]);

        // Iterate over keys before a key that doesn't exist in reverse, and doesn't have any closest key, should be empty iterator
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Reverse(&0))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![]);

        // Iterate over keys after a key that doesn't exist and doesn't have any closest key, should be empty iterator
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Forward(&4))
            .map(|r| r.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(kvs, vec![]);
    }

    #[test]
    fn write_batch() {
        let mut db = create_test_db(SCHEMAS);

        // Insert 2 kvs with write_batch
        let mut changes = HashMap::new();
        let btree: &mut BTreeMap<Vec<u8>, WriteOperation> =
            changes.entry(Dummy::METADATA.name).or_default();
        btree.insert(
            codec::serialize(&1u64),
            WriteOperation::Put(codec::serialize(&100u64)),
        );
        btree.insert(
            codec::serialize(&2u64),
            WriteOperation::Put(codec::serialize(&200u64)),
        );
        db.write_batch(changes).unwrap();
        assert_eq!(db.get::<Dummy>(&1).unwrap(), Some(100));
        assert_eq!(db.get::<Dummy>(&2).unwrap(), Some(200));

        let mut changes = HashMap::new();
        let btree: &mut BTreeMap<Vec<u8>, WriteOperation> =
            changes.entry(Dummy::METADATA.name).or_default();

        // Delete one of the previous keys, and add a new kv
        btree.insert(
            codec::serialize(&3u64),
            WriteOperation::Put(codec::serialize(&300u64)),
        );
        btree.insert(codec::serialize(&2u64), WriteOperation::Delete);

        db.write_batch(changes).unwrap();
        assert_eq!(db.get::<Dummy>(&1).unwrap(), Some(100));
        assert_eq!(db.get::<Dummy>(&2).unwrap(), None);
        assert_eq!(db.get::<Dummy>(&3).unwrap(), Some(300));
    }

    #[test]
    fn snapshot_key_exists_after_delete() {
        let mut db = create_test_db(SCHEMAS);

        db.put::<Dummy>(&1, &100).unwrap();
        db.put::<Dummy>(&2, &200).unwrap();
        assert!(db.exists::<Dummy>(&1).unwrap());
        assert!(db.exists::<Dummy>(&2).unwrap());

        let snapshot = db.snapshot();
        assert!(snapshot.exists::<Dummy>(&1).unwrap());
        assert!(snapshot.exists::<Dummy>(&2).unwrap());

        // Delete key
        db.delete::<Dummy>(&1).unwrap();

        // Key doesn't exist in database but exists in snapshot
        assert!(!db.exists::<Dummy>(&1).unwrap());
        assert!(db.exists::<Dummy>(&2).unwrap());
        assert!(snapshot.exists::<Dummy>(&1).unwrap());
        assert!(snapshot.exists::<Dummy>(&2).unwrap());
    }

    #[test]
    fn snapshot_key_doesnt_exist_after_put() {
        let mut db = create_test_db(SCHEMAS);

        db.put::<Dummy>(&1, &100).unwrap();
        db.put::<Dummy>(&2, &200).unwrap();
        assert!(db.exists::<Dummy>(&1).unwrap());
        assert!(db.exists::<Dummy>(&2).unwrap());

        let snapshot = db.snapshot();
        assert!(snapshot.exists::<Dummy>(&1).unwrap());
        assert!(snapshot.exists::<Dummy>(&2).unwrap());

        // Put new value
        db.put::<Dummy>(&3, &300).unwrap();

        // Key exist in database but doesn't exist in snapshot
        assert!(db.exists::<Dummy>(&3).unwrap());
        assert!(!snapshot.exists::<Dummy>(&3).unwrap());

        // Update existing value
        db.put::<Dummy>(&1, &1000).unwrap();

        // Db should be updated but snapshot should not change
        assert_eq!(db.get::<Dummy>(&1).unwrap(), Some(1000));
        assert_eq!(snapshot.get::<Dummy>(&1).unwrap(), Some(100));
    }

    #[test]
    fn drop_snapshot_after_dropping_database() {
        let db = create_test_db(SCHEMAS);

        let snapshot = db.snapshot();

        drop(db);

        drop(snapshot);
    }
}
