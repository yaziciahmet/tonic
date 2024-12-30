use tracing::Level;

pub fn initialize_tracing(level: Level) {
    tracing_subscriber::fmt().with_max_level(level).init();
}
