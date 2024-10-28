use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use rocksdb::{
    BlockBasedOptions, Cache, ColumnFamily, ColumnFamilyDescriptor, DBCompressionType,
    DBPinnableSlice, Options, ReadOptions, SnapshotWithThreadMode, WriteBatch, DB,
};

use crate::codec;
use crate::config::Config;
use crate::kv_store::{
    Changes, IteratorMode, KeyValueAccessor, KeyValueIterator, KeyValueMutator, WriteOperation,
};
use crate::schema::{Schema, SchemaName};
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
    ) -> impl Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>> + 'a {
        let cf = self.cf_handle(schema);

        self.inner
            .iterator_cf_opt(cf, self.read_opts(), rocks_db_mode)
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
        if let Some(snapshot) = snapshot {
            opts.set_snapshot(snapshot);
        }
        opts
    }
}

impl RocksDB<FullAccess> {
    #[cfg(feature = "test-helpers")]
    pub fn open_temp(config: Config, schemas: &[SchemaName]) -> RocksDB<FullAccess> {
        let path = tempfile::tempdir().unwrap();
        RocksDB::open(path, config, schemas)
    }

    pub fn open(
        path: impl AsRef<Path>,
        config: Config,
        schemas: &[SchemaName],
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
        // 128 MB of row cache
        let cache = Cache::new_lru_cache(config.max_cache_size as usize);
        opts.set_row_cache(&cache);

        let mut block_opts = BlockBasedOptions::default();
        // Default block size is 4 KB, but suggested as 16 KB.
        // https://github.com/facebook/rocksdb/wiki/memory-usage-in-rocksdb
        block_opts.set_block_size(16 * 1024);
        // Bloom filter to reduce disk I/O on reads
        block_opts.set_bloom_filter(10.0, false);
        // Store index and filter blocks in cache
        block_opts.set_cache_index_and_filter_blocks(true);
        // Don't evict L0 filter/index blocks from the cache
        block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        // Reduces bloom filter memory usage.
        block_opts.set_optimize_filters_for_memory(true);
        // 128 MB of block cache
        let cache = Cache::new_lru_cache(config.max_cache_size as usize);
        block_opts.set_block_cache(&cache);

        let cfs = schemas.iter().map(|name| {
            let mut cf_opts = Options::default();
            cf_opts.set_compression_type(DBCompressionType::Lz4);
            cf_opts.set_block_based_table_factory(&block_opts);

            ColumnFamilyDescriptor::new(*name, cf_opts)
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

    /// Create db snapshot.
    pub fn create_snapshot(&self) -> RocksDB<ViewOnlyAccess> {
        // Transmute snapshot to be static lifetime. Snapshot is dropped
        // safely before the RocksDB manually.
        let snapshot = unsafe {
            let snapshot = self.inner.snapshot();
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

    pub fn transaction(&self) -> InMemoryTransaction {
        InMemoryTransaction::new(&self)
    }

    pub(crate) fn commit_changes(&self, changes: Changes) -> Result<(), rocksdb::Error> {
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
}

impl<Access> KeyValueAccessor for RocksDB<Access>
where
    Access: ViewAccess,
{
    fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        Ok(self
            .raw_get(S::NAME, &key_bytes)?
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

        let values_bytes =
            self.raw_multi_get(S::NAME, keys_bytes.iter().map(|bytes| bytes.as_slice()));
        let mut values = Vec::with_capacity(values_bytes.len());
        for bytes in values_bytes {
            let value = bytes?.map(|bytes| codec::deserialize(bytes.as_ref()));
            values.push(value);
        }

        Ok(values)
    }

    fn exists<S: Schema>(&self, key: &S::Key) -> Result<bool, rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        Ok(self.raw_get(S::NAME, &key_bytes)?.is_some())
    }
}

impl<Access> KeyValueMutator for RocksDB<Access>
where
    Access: MutatorAccess,
{
    fn put<S: Schema>(&mut self, key: &S::Key, value: &S::Value) -> Result<(), rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        let value_bytes = codec::serialize(value);
        self.raw_put(S::NAME, &key_bytes, &value_bytes)
    }

    fn delete<S: Schema>(&mut self, key: &S::Key) -> Result<(), rocksdb::Error> {
        let key_bytes = codec::serialize(key);
        self.raw_delete(S::NAME, &key_bytes)
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

        self.raw_iterator(S::NAME, rocks_db_mode).map(|result| {
            result.map(|(key_bytes, value_bytes)| {
                let key = codec::deserialize(&key_bytes);
                let value = codec::deserialize(&value_bytes);
                (key, value)
            })
        })
    }
}

#[cfg(feature = "test-helpers")]
pub fn create_test_db() -> RocksDB<FullAccess> {
    let config = Config {
        max_open_files: 8,
        max_cache_size: 1024 * 1024,
        max_total_wal_size: 2 * 1024 * 1024,
    };
    RocksDB::open_temp(config, &[crate::schema::Dummy::NAME])
}

#[cfg(test)]
mod tests {
    use crate::kv_store::{IteratorMode, KeyValueAccessor, KeyValueIterator, KeyValueMutator};
    use crate::schema::Dummy;

    use super::{create_test_db, FullAccess, RocksDB};

    const ORDERED_KVS: [(u64, u64); 4] = [(0, 100), (1, 200), (2, 200), (3, 300)];

    fn create_populated_db() -> RocksDB<FullAccess> {
        let mut db = create_test_db();

        // Populate and check values
        for (key, value) in &ORDERED_KVS {
            assert!(!db.exists::<Dummy>(key).unwrap());
            assert_eq!(db.get::<Dummy>(key).unwrap(), None);

            db.put::<Dummy>(key, value).unwrap();

            assert!(db.exists::<Dummy>(key).unwrap());
            assert_eq!(db.get::<Dummy>(key).unwrap(), Some(*value));
        }

        db
    }

    #[test]
    fn put_and_get() {
        create_populated_db();
    }

    #[test]
    fn put_and_delete() {
        let mut db = create_populated_db();

        // Delete a key and validate deletion
        let kv0 = ORDERED_KVS[0];
        db.delete::<Dummy>(&kv0.0).unwrap();

        assert!(!db.exists::<Dummy>(&kv0.0).unwrap());
        assert_eq!(db.get::<Dummy>(&kv0.0).unwrap(), None);

        let count = db.iterator::<Dummy>(IteratorMode::Start).count();
        assert_eq!(count, ORDERED_KVS.len() - 1);
    }

    #[test]
    fn put_and_multi_get() {
        let db = create_populated_db();

        // Get all keys
        let (ordered_keys, ordered_values): (Vec<_>, Vec<_>) =
            ORDERED_KVS.into_iter().map(|(k, v)| (k, Some(v))).unzip();
        assert_eq!(db.multi_get::<Dummy>(ordered_keys).unwrap(), ordered_values);

        // Only get even keys
        let (ordered_keys, ordered_values): (Vec<_>, Vec<_>) = ORDERED_KVS
            .into_iter()
            .filter_map(|(k, v)| if k % 2 == 0 { Some((k, Some(v))) } else { None })
            .unzip();
        assert_eq!(db.multi_get::<Dummy>(ordered_keys).unwrap(), ordered_values);
    }

    #[test]
    fn iterator() {
        let db = create_populated_db();

        // Validate entry count
        let count = db.iterator::<Dummy>(IteratorMode::Start).count();
        assert_eq!(count, ORDERED_KVS.len());

        let ordered_kvs = ORDERED_KVS.clone();
        // Validate each key-value entry in ascending order
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Start)
            .map(|kv| kv.unwrap())
            .enumerate()
            .collect::<Vec<_>>();
        for (idx, kv) in kvs {
            assert_eq!(kv, ordered_kvs[idx]);
        }

        // Validate each key-value entry in descending order
        let mut ordered_kvs_rev = ORDERED_KVS.clone();
        ordered_kvs_rev.reverse();
        let kvs = db
            .iterator::<Dummy>(IteratorMode::End)
            .map(|kv| kv.unwrap())
            .enumerate()
            .collect::<Vec<_>>();
        for (idx, kv) in kvs {
            assert_eq!(kv, ordered_kvs_rev[idx]);
        }

        // Iterate starting from the 2nd index of kvs
        let ordered_kvs_sliced = &ORDERED_KVS[2..];
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Forward(&ordered_kvs_sliced[0].0))
            .map(|kv| kv.unwrap())
            .enumerate()
            .collect::<Vec<_>>();
        for (idx, kv) in kvs {
            assert_eq!(kv, ordered_kvs_sliced[idx]);
        }

        // Iterate in reverse starting from the LEN-2 index of kvs
        let mut ordered_kvs_rev = ORDERED_KVS.clone();
        ordered_kvs_rev.reverse();
        let ordered_kvs_rev_sliced = &ordered_kvs_rev[1..];
        let kvs = db
            .iterator::<Dummy>(IteratorMode::Reverse(&ordered_kvs_rev_sliced[0].0))
            .map(|kv| kv.unwrap())
            .enumerate()
            .collect::<Vec<_>>();
        for (idx, kv) in kvs {
            assert_eq!(kv, ordered_kvs_rev_sliced[idx]);
        }
    }

    #[test]
    fn snapshot_gets_key_after_delete() {
        let mut db = create_populated_db();

        let key = 0;
        assert!(db.exists::<Dummy>(&key).unwrap());

        let snapshot = db.create_snapshot();
        assert!(snapshot.exists::<Dummy>(&key).unwrap());

        db.delete::<Dummy>(&key).unwrap();

        // Key doesn't exist in database but exists in snapshot
        assert!(!db.exists::<Dummy>(&key).unwrap());
        assert!(snapshot.exists::<Dummy>(&key).unwrap());
    }

    #[test]
    fn drop_snapshot_after_dropping_database() {
        let db = create_populated_db();

        let snapshot = db.create_snapshot();

        drop(db);

        drop(snapshot);
    }
}
