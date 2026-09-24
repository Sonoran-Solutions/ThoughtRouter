use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result, bail};
use rusqlite::Connection;

/// Ordered migrations; index + 1 is the resulting `PRAGMA user_version`.
/// Never edit a released migration: append a new one.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

/// Single-user local database. One connection behind a mutex is plenty for
/// a personal corpus; callers must not hold the lock across network calls.
pub struct Db {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening database {}", path.display()))?;
        Self::init(conn, Some(path.to_path_buf()))
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?, None)
    }

    fn init(mut conn: Connection, path: Option<PathBuf>) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        if path.is_some() {
            // WAL lets the background worker write while the UI reads; FULL
            // sync because "the thought is saved" must survive power loss.
            let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
            conn.pragma_update(None, "synchronous", "FULL")?;
        }
        migrate(&mut conn)?;
        Ok(Db {
            conn: Mutex::new(conn),
            path,
        })
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

pub fn schema_version(conn: &Connection) -> Result<usize> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    Ok(v as usize)
}

pub fn migrate(conn: &mut Connection) -> Result<()> {
    let current = schema_version(conn)?;
    if current > MIGRATIONS.len() {
        bail!(
            "database schema v{current} is newer than this app (v{}); refusing to open",
            MIGRATIONS.len()
        );
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("applying migration {}", i + 1))?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_empty_db_to_latest() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(schema_version(&db.conn()).unwrap(), MIGRATIONS.len());
    }

    #[test]
    fn migration_is_idempotent_on_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tr.db");
        drop(Db::open(&path).unwrap());
        let db = Db::open(&path).unwrap();
        assert_eq!(schema_version(&db.conn()).unwrap(), MIGRATIONS.len());
    }

    #[test]
    fn refuses_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tr.db");
        drop(Db::open(&path).unwrap());
        {
            let c = Connection::open(&path).unwrap();
            c.pragma_update(None, "user_version", 999).unwrap();
        }
        assert!(Db::open(&path).is_err());
    }

    #[test]
    fn fts5_is_available() {
        let db = Db::open_in_memory().unwrap();
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM captures_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
