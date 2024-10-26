#[derive(Clone, Debug)]
pub struct Config {
    /// Maximum number of open file descriptors.
    /// OSX limit is 256, other OS'es have it higher.
    /// Set -1 for no limit. Defaults to 512.
    pub max_open_files: i32,
    /// Maximum total WAL size before WAL is flushed to disk.
    /// Defaults to 256 MB.
    pub max_total_wal_size: u64,
    /// Maximum cache size. Defaults to 128 MB.
    pub max_cache_size: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_open_files: 512,
            max_total_wal_size: 256 * 1024 * 1024,
            max_cache_size: 128 * 1024 * 1024,
        }
    }
}
