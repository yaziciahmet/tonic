use tracing::Level;

pub fn initialize_tracing(level: Level) {
    let _ = tracing_subscriber::fmt().with_max_level(level).try_init();
}
