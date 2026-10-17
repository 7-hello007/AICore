use std::path::{Path, PathBuf};
use std::sync::Mutex;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    memory::{profile::UserProfile, record::HistoricalRecord},
    traits::memory_store::IMemoryStore,
};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

/// Current schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Migration of each version SQL. The key is the target version and the value is DDL。
const MIGRATIONS: &[(u32, &str)] = &[(
    1,
    "CREATE TABLE IF NOT EXISTS kv (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS records (
            record_id TEXT PRIMARY KEY,
            timestamp_unix_ms INTEGER NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_records_ts ON records(timestamp_unix_ms);",
)];

const KV_SCHEMA_VERSION: &str = "schema_version";
const KV_PROFILE: &str = "profile";

/// Local storage based on SQLite.
///
/// - Service type.
/// - Schema version+ migration.
/// - Supports pressing timestamp to delete.
pub struct SqliteMemoryStore {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl SqliteMemoryStore {
    /// Worker reads never create a DB, migrate a schema or change PRAGMAs.
    pub fn open_read_only(path: impl Into<PathBuf>) -> ContractResult<Self> {
        let path = path.into();
        let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(map_sqlite)?;
        Ok(Self {
            path,
            conn: Mutex::new(conn),
        })
    }
    /// Opens or creates a database.
    ///
    /// - If the document does not exist, the migration will be created and applied.
    /// - If the file exists, check the schema version and move as required.
    pub fn open(path: impl Into<PathBuf>) -> ContractResult<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ContractError::AdapterUnavailable(format!("create dir {}: {}", parent.display(), e))
            })?;
        }

        let conn = Connection::open(&path).map_err(|e| {
            ContractError::AdapterUnavailable(format!("open sqlite {}: {}", path.display(), e))
        })?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(map_sqlite)?;
        let store = Self {
            path,
            conn: Mutex::new(conn),
        };
        store.apply_migrations()?;
        Ok(store)
    }

    /// Opens the memory database (testing).
    pub fn open_in_memory() -> ContractResult<Self> {
        let conn = Connection::open_in_memory().map_err(|e| {
            ContractError::AdapterUnavailable(format!("open in-memory sqlite: {}", e))
        })?;

        let store = Self {
            path: PathBuf::from(":memory:"),
            conn: Mutex::new(conn),
        };

        store.apply_migrations()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn import_json_snapshot(
        &self,
        source: &str,
        records: &[HistoricalRecord],
        profile: &UserProfile,
    ) -> ContractResult<super::migrate::MigrationReport> {
        let serialized = serde_json::to_string(&(records, profile))
            .map_err(|e| ContractError::InvalidRequest(e.to_string()))?;
        // Bookkeeping must not retain another copy of historical/profile data
        // after consent changes or retention removes the real records.
        let fingerprint = format!("{:x}", Sha256::digest(serialized.as_bytes()));
        let marker = format!("json_migration:{:x}", Sha256::digest(source.as_bytes()));
        self.with_conn(|conn| {
            let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate).map_err(map_sqlite)?;
            let completed: Option<String> = tx.query_row("SELECT value FROM kv WHERE key=?1", [&marker], |row| row.get(0)).optional().map_err(map_sqlite)?;
            if completed.as_deref() == Some(fingerprint.as_str()) { return Ok(Default::default()); }
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0)).map_err(map_sqlite)?;
            if count > 0 {
                let mut overlaps = false;
                for record in records {
                    let found: Option<i64> = tx.query_row("SELECT 1 FROM records WHERE record_id=?1", [&record.record_id], |row| row.get(0)).optional().map_err(map_sqlite)?;
                    overlaps |= found.is_some();
                }
                // Preserve the existing refusal to merge an unrelated database.
                // An old interrupted import is recognized by shared record IDs.
                if !overlaps { return Ok(super::migrate::MigrationReport { skipped: true, skip_reason: Some("target sqlite already has unrelated records".into()), ..Default::default() }); }
            }
            let mut report = super::migrate::MigrationReport::default();
            for record in records {
                let existing: Option<String> = tx.query_row("SELECT payload_json FROM records WHERE record_id=?1", [&record.record_id], |row| row.get(0)).optional().map_err(map_sqlite)?;
                if let Some(payload) = existing {
                    let previous: HistoricalRecord = serde_json::from_str(&payload).map_err(|e| ContractError::InvalidRequest(e.to_string()))?;
                    if previous != *record { return Err(ContractError::InvalidRequest(format!("migration record conflict: {}", record.record_id))); }
                } else {
                    let payload = serde_json::to_string(record).map_err(|e| ContractError::InvalidRequest(e.to_string()))?;
                    tx.execute("INSERT INTO records (record_id,timestamp_unix_ms,payload_json) VALUES (?1,?2,?3)", params![record.record_id, record.timestamp_unix_ms as i64, payload]).map_err(map_sqlite)?;
                    report.records_migrated += 1;
                }
            }
            let previous: Option<String> = tx.query_row("SELECT value FROM kv WHERE key=?1", [KV_PROFILE], |row| row.get(0)).optional().map_err(map_sqlite)?;
            let previous: Option<UserProfile> = previous.map(|value| serde_json::from_str(&value)).transpose().map_err(|e| ContractError::InvalidRequest(e.to_string()))?;
            if profile.updated_at_unix_secs > 0 && previous.as_ref().map_or(true, |old| old.updated_at_unix_secs < profile.updated_at_unix_secs) {
                let payload = serde_json::to_string(profile).map_err(|e| ContractError::InvalidRequest(e.to_string()))?;
                tx.execute("INSERT INTO kv (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![KV_PROFILE, payload]).map_err(map_sqlite)?;
                report.profile_migrated = true;
            }
            tx.execute("INSERT INTO kv (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![marker, fingerprint]).map_err(map_sqlite)?;
            tx.commit().map_err(map_sqlite)?;
            Ok(report)
        })
    }

    fn with_conn<F, T>(&self, f: F) -> ContractResult<T>
    where
        F: FnOnce(&Connection) -> ContractResult<T>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ContractError::AdapterUnavailable("sqlite lock poisoned".into()))?;
        f(&conn)
    }

    fn apply_migrations(&self) -> ContractResult<()> {
        self.with_conn(|conn| {
            // Keep DDL, timestamp conversion and the schema marker together.
            // A failed or interrupted migration must leave the previous schema intact.
            let transaction = conn.unchecked_transaction().map_err(map_sqlite)?;
            // To read schema_version
            transaction
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS kv (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );",
                )
                .map_err(map_sqlite)?;

            let current = read_schema_version(&transaction)?;

            for (version, sql) in MIGRATIONS {
                if *version > current {
                    transaction.execute_batch(sql).map_err(map_sqlite)?;
                    write_schema_version(&transaction, *version)?;
                }
            }

            // v1 of the product stored record timestamps in a column named
            // `timestamp_unix_secs`, while the payload itself already used
            // millisecond precision. Upgrade that legacy schema in-place.
            let version_after_base = read_schema_version(&transaction)?;
            if version_after_base < 2 {
                let mut has_legacy_column = false;
                let mut stmt = transaction
                    .prepare("PRAGMA table_info(records)")
                    .map_err(map_sqlite)?;
                let rows = stmt
                    .query_map([], |row| row.get::<_, String>(1))
                    .map_err(map_sqlite)?;
                for row in rows {
                    if row.map_err(map_sqlite)? == "timestamp_unix_secs" {
                        has_legacy_column = true;
                        break;
                    }
                }
                if has_legacy_column {
                    transaction
                        .execute_batch(
                            "DROP INDEX IF EXISTS idx_records_ts;
                         ALTER TABLE records RENAME COLUMN timestamp_unix_secs TO timestamp_unix_ms;
                         UPDATE records SET timestamp_unix_ms = timestamp_unix_ms * 1000;
                         CREATE INDEX IF NOT EXISTS idx_records_ts ON records(timestamp_unix_ms);",
                        )
                        .map_err(map_sqlite)?;
                } else {
                    transaction.execute_batch(
                        "CREATE INDEX IF NOT EXISTS idx_records_ts ON records(timestamp_unix_ms);",
                    )
                    .map_err(map_sqlite)?;
                }
                write_schema_version(&transaction, 2)?;
            }

            transaction.commit().map_err(map_sqlite)
        })
    }
}

fn map_sqlite(e: rusqlite::Error) -> ContractError {
    ContractError::AdapterUnavailable(format!("sqlite: {}", e))
}

fn read_schema_version(conn: &Connection) -> ContractResult<u32> {
    let mut stmt = conn
        .prepare("SELECT value FROM kv WHERE key = ?1")
        .map_err(map_sqlite)?;
    let result: Result<String, _> = stmt.query_row(params![KV_SCHEMA_VERSION], |row| row.get(0));
    match result {
        Ok(s) => Ok(s.parse().unwrap_or(0)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(e) => Err(map_sqlite(e)),
    }
}

fn write_schema_version(conn: &Connection, version: u32) -> ContractResult<()> {
    conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KV_SCHEMA_VERSION, version.to_string()],
    )
    .map_err(map_sqlite)?;
    Ok(())
}

impl IMemoryStore for SqliteMemoryStore {
    fn try_process_source(
        &self,
        _: usize,
    ) -> ContractResult<aicore_contracts::traits::memory_store::ProcessMemorySource> {
        let path = self.path.to_string_lossy();
        if path.is_empty() || path == ":memory:" || path.starts_with("file:") {
            return Err(ContractError::NotSupported(
                "SQLite source is not a reopenable file path for managed IPC".into(),
            ));
        }
        Ok(
            aicore_contracts::traits::memory_store::ProcessMemorySource::Sqlite {
                path: self.path.clone(),
            },
        )
    }
    fn append_record(&self, record: &HistoricalRecord) -> ContractResult<()> {
        let payload = serde_json::to_string(record)
            .map_err(|e| ContractError::AdapterUnavailable(format!("serialize record: {}", e)))?;

        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO records (record_id, timestamp_unix_ms, payload_json)
                 VALUES (?1, ?2, ?3)",
                params![record.record_id, record.timestamp_unix_ms as i64, payload],
            )
            .map_err(map_sqlite)?;
            Ok(())
        })
    }

    fn load_records(&self, limit: usize) -> ContractResult<Vec<HistoricalRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT payload_json FROM records
                     ORDER BY timestamp_unix_ms DESC
                     LIMIT ?1",
                )
                .map_err(map_sqlite)?;

            let rows = stmt
                .query_map(params![limit as i64], |row| {
                    let payload: String = row.get(0)?;
                    Ok(payload)
                })
                .map_err(map_sqlite)?;

            let mut result: Vec<HistoricalRecord> = Vec::new();
            for r in rows {
                let payload = r.map_err(map_sqlite)?;
                match serde_json::from_str::<HistoricalRecord>(&payload) {
                    Ok(rec) => result.push(rec),
                    Err(e) => {
                        tracing::warn!(
                            component = "sqlite_memory",
                            event = "record_deserialize_failed",
                            error = %e,
                            "skipping malformed record"
                        );
                    },
                }
            }
            // Invert the results in time (old and new),
            // The syntax of InMemoryStore/JsonFileStore.
            result.reverse();
            Ok(result)
        })
    }

    fn clear_history(&self) -> ContractResult<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM records", [])
                .map_err(map_sqlite)?;
            Ok(())
        })
    }

    fn record_count(&self) -> ContractResult<usize> {
        self.with_conn(|conn| {
            let n: i64 = conn
                .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
                .map_err(map_sqlite)?;
            Ok(n as usize)
        })
    }

    fn load_profile(&self) -> ContractResult<UserProfile> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare("SELECT value FROM kv WHERE key = ?1")
                .map_err(map_sqlite)?;
            let result: Result<String, _> = stmt.query_row(params![KV_PROFILE], |row| row.get(0));
            match result {
                Ok(s) => serde_json::from_str::<UserProfile>(&s).map_err(|e| {
                    ContractError::AdapterUnavailable(format!("deserialize profile: {}", e))
                }),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(UserProfile::default()),
                Err(e) => Err(map_sqlite(e)),
            }
        })
    }

    fn save_profile(&self, profile: &UserProfile) -> ContractResult<()> {
        let payload = serde_json::to_string(profile)
            .map_err(|e| ContractError::AdapterUnavailable(format!("serialize profile: {}", e)))?;

        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO kv (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![KV_PROFILE, payload],
            )
            .map_err(map_sqlite)?;
            Ok(())
        })
    }

    fn delete_records_before(&self, cutoff_unix_ms: u64) -> ContractResult<usize> {
        self.with_conn(|conn| {
            let n = conn
                .execute(
                    "DELETE FROM records WHERE timestamp_unix_ms < ?1",
                    params![cutoff_unix_ms as i64],
                )
                .map_err(map_sqlite)?;

            Ok(n)
        })
    }

    fn schema_version(&self) -> ContractResult<u32> {
        self.with_conn(read_schema_version)
    }

    fn store_name(&self) -> &'static str {
        "sqlite"
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn p1_completion_marker_does_not_retain_replaced_profile() {
        let store = SqliteMemoryStore::open_in_memory().unwrap();
        let profile = UserProfile {
            version: "private-old-profile".into(),
            updated_at_unix_secs: 100,
            ..Default::default()
        };
        store
            .import_json_snapshot("fixture-source", &[], &profile)
            .unwrap();
        store.save_profile(&UserProfile::default()).unwrap();
        let values = store
            .with_conn(|conn| {
                let mut statement = conn.prepare("SELECT value FROM kv").map_err(map_sqlite)?;
                let rows = statement
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(map_sqlite)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(map_sqlite)
            })
            .unwrap();
        assert!(
            !values.join(" ").contains("private-old-profile"),
            "migration bookkeeping must not retain an extra copy of replaced personal data"
        );
    }

    #[test]
    fn failed_legacy_conversion_rolls_back_schema_and_data() {
        let path = std::env::temp_dir().join(format!(
            "aicore-v4-migration-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO kv VALUES ('schema_version', '1');
                 CREATE TABLE records (record_id TEXT PRIMARY KEY,
                     timestamp_unix_secs INTEGER NOT NULL, payload_json TEXT NOT NULL);
                 INSERT INTO records VALUES ('existing', 123, '{}');
                 CREATE INDEX idx_records_ts ON records(timestamp_unix_secs);
                 CREATE TRIGGER reject_conversion BEFORE UPDATE ON records
                     BEGIN SELECT RAISE(ABORT, 'blocked conversion'); END;",
            )
            .unwrap();
        }

        assert!(SqliteMemoryStore::open(&path).is_err());
        {
            let conn = Connection::open(&path).unwrap();
            let version: String = conn
                .query_row(
                    "SELECT value FROM kv WHERE key='schema_version'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(version, "1");
            let timestamp: i64 = conn
                .query_row("SELECT timestamp_unix_secs FROM records", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(timestamp, 123);
            conn.execute_batch("DROP TRIGGER reject_conversion;")
                .unwrap();
        }
        let store = SqliteMemoryStore::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 2);
        let conn = Connection::open(&path).unwrap();
        let timestamp: i64 = conn
            .query_row("SELECT timestamp_unix_ms FROM records", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(timestamp, 123_000);
        drop(conn);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
