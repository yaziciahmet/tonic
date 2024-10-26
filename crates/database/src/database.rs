use std::path::Path;

pub use rocksdb::Error as RocksDbError;
use rocksdb::{
    BlockBasedOptions, Cache, ColumnFamily, ColumnFamilyDescriptor, DBCompressionType, Options,
    ReadOptions, SnapshotWithThreadMode, DB,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::config::Config;
use crate::schema::{Schema, SchemaName};

/// `Database` is a wrapper around `rocksdb::DB` to provide
/// `Schema` compatible API with auto bincode serialization.
pub struct Database {
    inner: DB,
}

impl Database {
    #[cfg(feature = "test-helpers")]
    pub fn open_temp(config: Config, schema_names: &[SchemaName]) -> Self {
        let path = tempfile::tempdir().unwrap();
        Self::open(path, config, schema_names)
    }

    pub fn open(path: impl AsRef<Path>, config: Config, schema_names: &[SchemaName]) -> Self {
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

        let cfs = schema_names.iter().map(|name| {
            let mut cf_opts = Options::default();
            cf_opts.set_compression_type(DBCompressionType::Lz4);
            cf_opts.set_block_based_table_factory(&block_opts);

            ColumnFamilyDescriptor::new(*name, cf_opts)
        });

        let inner = DB::open_cf_descriptors(&opts, path, cfs)
            .expect("Failed open RocksDB with cf descriptors");

        Self { inner }
    }

    /// Get a value from the schema by key
    pub fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, RocksDbError> {
        let cf = self.cf_handle(S::NAME);

        let key_serialized = Self::serialize(key);
        let value_serialized = self.inner.get_pinned_cf(cf, key_serialized)?;

        Ok(value_serialized.map(|v| Self::deserialize(v.as_ref())))
    }

    /// Get multiple values from the schema by list of keys.
    /// When using this method, ensure that keys are already sorted
    /// to avoid unexpected behaviour.
    pub fn multi_get<S: Schema>(
        &self,
        keys: impl IntoIterator<Item = S::Key>,
    ) -> Result<Vec<Option<S::Value>>, RocksDbError> {
        let cf = self.cf_handle(S::NAME);

        let keys_serialized = keys
            .into_iter()
            .map(|key| Self::serialize(&key))
            .collect::<Vec<_>>();
        let values_serialized = self.inner.batched_multi_get_cf(cf, &keys_serialized, true);

        let mut values = Vec::with_capacity(values_serialized.len());
        for value_serialized in values_serialized {
            let value: Option<S::Value> = value_serialized?.map(|slice| Self::deserialize(&slice));
            values.push(value);
        }

        Ok(values)
    }

    /// Put a key-value pair into the schema
    pub fn put<S: Schema>(&self, key: &S::Key, value: &S::Value) -> Result<(), RocksDbError> {
        let cf = self.cf_handle(S::NAME);

        let key_serialized = Self::serialize(key);
        let value_serialized = Self::serialize(value);

        self.inner.put_cf(cf, key_serialized, value_serialized)
    }

    /// Delete a key from the schema
    pub fn delete<S: Schema>(&self, key: &S::Key) -> Result<(), RocksDbError> {
        let cf = self.cf_handle(S::NAME);

        let key_serialized = Self::serialize(key);

        self.inner.delete_cf(cf, key_serialized)
    }

    pub fn exists<S: Schema>(&self, key: &S::Key) -> Result<bool, RocksDbError> {
        let cf = self.cf_handle(S::NAME);

        let key_serialized = Self::serialize(key);
        let value_serialized = self.inner.get_pinned_cf(cf, key_serialized)?;

        Ok(value_serialized.is_some())
    }

    /// Returns an iterator with the provided `mode`. If `mode` is `Forward` or `Reverse`,
    /// key iteration is inclusive.
    pub fn iterator<'a, S: Schema>(
        &'a self,
        mode: IteratorMode<S>,
    ) -> impl Iterator<Item = Result<(S::Key, S::Value), RocksDbError>> + 'a {
        let cf = self.cf_handle(S::NAME);

        // We define this here because key_serialized must
        // must live at least as much as rocks_db_mode
        let key_serialized;
        let rocks_db_mode = match mode {
            IteratorMode::Start => rocksdb::IteratorMode::Start,
            IteratorMode::End => rocksdb::IteratorMode::End,
            IteratorMode::Forward(key) => {
                key_serialized = Self::serialize(&key);
                rocksdb::IteratorMode::From(&key_serialized, rocksdb::Direction::Forward)
            }
            IteratorMode::Reverse(key) => {
                key_serialized = Self::serialize(&key);
                rocksdb::IteratorMode::From(&key_serialized, rocksdb::Direction::Reverse)
            }
        };

        self.inner.iterator_cf(cf, rocks_db_mode).map(|result| {
            result.map(|(key_serialized, value_serialized)| {
                let key: S::Key = Self::deserialize(&key_serialized);
                let value: S::Value = Self::deserialize(&value_serialized);
                (key, value)
            })
        })
    }

    pub fn create_snapshot<'a>(&'a self) -> rocksdb::SnapshotWithThreadMode<'a, DB> {
        self.inner.snapshot()
    }

    /// Asserts existence of column family and returns it.
    fn cf_handle(&self, name: &str) -> &ColumnFamily {
        self.inner
            .cf_handle(name)
            .unwrap_or_else(|| panic!("Received non-existing schema `{name}`"))
    }

    fn serialize<T: Serialize>(item: &T) -> Vec<u8> {
        bincode::serialize(item).expect("DB serialization can not fail")
    }

    fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> T {
        bincode::deserialize(bytes).expect("DB deserialization can not fail")
    }
}

/// `IteratorMode` is a `Schema` wrapped `rocksdb::IteratorMode`.
#[derive(Debug)]
pub enum IteratorMode<S: Schema> {
    Start,
    End,
    Forward(S::Key),
    Reverse(S::Key),
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use crate::config::Config;
    use crate::schema::Schema;

    use super::{Database, IteratorMode};

    crate::define_schema!(
        /// A very very dummy schema
        (Dummy) u64 => u64
    );

    const ORDERED_KVS: [(u64, u64); 4] = [(0, 100), (1, 200), (2, 200), (3, 300)];

    fn init_populated_db() -> Database {
        let config = Config {
            max_open_files: 8,
            max_cache_size: 1024 * 1024,
            max_total_wal_size: 2 * 1024 * 1024,
        };
        let db = Database::open_temp(config, &[Dummy::NAME, TestBlocks::NAME]);

        // Populate and check values
        for (key, value) in &ORDERED_KVS {
            assert!(!db.exists::<Dummy>(key).unwrap());
            assert_eq!(db.get::<Dummy>(key).unwrap(), None);

            db.put::<Dummy>(key, value).unwrap();

            assert!(db.exists::<Dummy>(key).unwrap());
            assert_eq!(db.get::<Dummy>(key).unwrap(), Some(*value));
        }

        // Validate entry count
        let count = db.iterator::<Dummy>(IteratorMode::Start).count();
        assert_eq!(count, ORDERED_KVS.len());

        let (ordered_keys, ordered_values): (Vec<_>, Vec<_>) =
            ORDERED_KVS.into_iter().map(|(k, v)| (k, Some(v))).unzip();
        assert_eq!(db.multi_get::<Dummy>(ordered_keys).unwrap(), ordered_values);

        db
    }

    #[test]
    fn simple() {
        init_populated_db();
    }

    #[test]
    fn delete() {
        let db = init_populated_db();

        // Delete a key and validate deletion
        let kv0 = ORDERED_KVS[0];
        db.delete::<Dummy>(&kv0.0).unwrap();

        assert!(!db.exists::<Dummy>(&kv0.0).unwrap());
        assert_eq!(db.get::<Dummy>(&kv0.0).unwrap(), None);

        let count = db.iterator::<Dummy>(IteratorMode::Start).count();
        assert_eq!(count, ORDERED_KVS.len() - 1);
    }

    #[test]
    fn multi_get() {
        let db = init_populated_db();

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
        let db = init_populated_db();

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
            .iterator::<Dummy>(IteratorMode::Forward(ordered_kvs_sliced[0].0))
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
            .iterator::<Dummy>(IteratorMode::Reverse(ordered_kvs_rev_sliced[0].0))
            .map(|kv| kv.unwrap())
            .enumerate()
            .collect::<Vec<_>>();
        for (idx, kv) in kvs {
            assert_eq!(kv, ordered_kvs_rev_sliced[idx]);
        }
    }

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
