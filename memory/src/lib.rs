pub mod context_builder;
pub mod personal;
pub mod retention;
pub mod store;

pub use context_builder::build_memory_context;
pub use personal::personalized_context;
pub use retention::RetentionScheduler;
pub use store::{
    default_json_history_path, default_json_profile_path, default_sqlite_path,
    migrate_json_to_sqlite, migrate_with_defaults, InMemoryStore, JsonFileStore, MigrationReport,
    SqliteMemoryStore, CURRENT_SCHEMA_VERSION,
};
