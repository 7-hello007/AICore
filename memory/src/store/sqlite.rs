use std::path::{Path, PathBuf};
use std::sync::Mutex;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    memory::{profile::UserProfile, record::HistoricalRecord},
    traits::memory_store::IMemoryStore,
};
use rusqlite::{params, Connection};

/// 当前 schema version。
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// 每个版本的迁移 SQL。键是目标版本，值是 DDL。
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

/// 基于 SQLite 的本地存储。
///
/// - 事务型。
/// - Schema version + 迁移。
/// - 支持按 timestamp 删除。
pub struct SqliteMemoryStore {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl SqliteMemoryStore {
    /// 打开或创建数据库。
    ///
    /// - 若文件不存在，会创建并应用迁移。
    /// - 若文件存在，会检查 schema version 并按需迁移。
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

    /// 打开内存数据库（测试用）。
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
            // 保证 kv 表存在，才可读取 schema_version
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS kv (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );",
            )
            .map_err(map_sqlite)?;

            let current = read_schema_version(conn)?;

            for (version, sql) in MIGRATIONS {
                if *version > current {
                    conn.execute_batch(sql).map_err(map_sqlite)?;
                    write_schema_version(conn, *version)?;
                }
            }

            // v1 of the product stored record timestamps in a column named
            // `timestamp_unix_secs`, while the payload itself already used
            // millisecond precision. Upgrade that legacy schema in-place.
            let version_after_base = read_schema_version(conn)?;
            if version_after_base < 2 {
                let mut has_legacy_column = false;
                let mut stmt = conn
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
                    conn.execute_batch(
                        "DROP INDEX IF EXISTS idx_records_ts;
                         ALTER TABLE records RENAME COLUMN timestamp_unix_secs TO timestamp_unix_ms;
                         UPDATE records SET timestamp_unix_ms = timestamp_unix_ms * 1000;
                         CREATE INDEX IF NOT EXISTS idx_records_ts ON records(timestamp_unix_ms);",
                    )
                    .map_err(map_sqlite)?;
                } else {
                    conn.execute_batch(
                        "CREATE INDEX IF NOT EXISTS idx_records_ts ON records(timestamp_unix_ms);",
                    )
                    .map_err(map_sqlite)?;
                }
                write_schema_version(conn, 2)?;
            }

            Ok(())
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
            // 反转，使结果按时间升序（旧 → 新），
            // 与 InMemoryStore / JsonFileStore 的语义一致。
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
