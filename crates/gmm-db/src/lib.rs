// SPDX-License-Identifier: MIT OR Apache-2.0

//! GMM's local database: one SQLite file in the app data folder (`%APPDATA%\GMM\gmm.db`).
//!
//! It holds what GMM can't recompute from disk: the mods it installed, the archives it keeps
//! in its download cache, the user's settings (manual game folders, the XXMI folder and exe
//! they confirmed), and cached GameBanana responses. Setup states are never stored; they are
//! worked out fresh each time (docs/02-architecture.md, "Data model").
//!
//! The schema lives in `migrations/` and is applied on open. The database file is located
//! through [`gmm_fsguard`] and must sit in a read-write root; SQLite then opens it (and its
//! `-wal`/`-shm` siblings) itself, which is the one file access that doesn't go through the
//! guard's methods.

mod downloads;
mod error;
mod http_cache;
mod mods;
mod settings;

use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gmm_fsguard::{Guard, ScopedPath};
use sqlx::SqlSafeStr;
use sqlx::migrate::{Migration, MigrationType, Migrator};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

pub use downloads::{CachedDownload, NewCachedDownload};
pub use error::DbError;
pub use mods::{InstalledMod, NewInstalledMod};
pub use settings::ManualGameFolder;

/// The database's file name inside GMM's data folder.
pub const DB_FILE_NAME: &str = "gmm.db";

/// Cached API responses older than this are deleted on open. The longest freshness window
/// any endpoint asks for is a day.
const HTTP_CACHE_KEEP: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// At most this many cached API responses are kept; the oldest go first.
const HTTP_CACHE_MAX_ROWS: i64 = 5_000;

const MIGRATIONS: &[(i64, &str, &str)] =
    &[(1, "initial", include_str!("../migrations/0001_initial.sql"))];

/// A handle to the database. Cheap to clone; clones share one connection pool.
#[derive(Debug, Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Opens (or creates) the database at `file`, which must be in a read-write root, and
    /// brings its schema up to date.
    pub async fn open(guard: &Guard, file: &ScopedPath) -> Result<Self, DbError> {
        let path = guard.os_path_for_library(file)?;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        Self::init(pool).await
    }

    /// A private database that lives only as long as the handle. For tests.
    pub async fn open_in_memory() -> Result<Self, DbError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // Every connection to `:memory:` is its own database, so keep exactly one, forever.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;
        Self::init(pool).await
    }

    async fn init(pool: SqlitePool) -> Result<Self, DbError> {
        migrator().run(&pool).await?;
        let db = Self { pool };
        db.prune_http_cache(HTTP_CACHE_KEEP, HTTP_CACHE_MAX_ROWS)
            .await?;
        Ok(db)
    }

    /// Highest schema version applied to this database.
    pub async fn schema_version(&self) -> Result<i64, DbError> {
        let version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
                .fetch_one(&self.pool)
                .await?;
        Ok(version.unwrap_or(0))
    }

    /// Waits for pending writes and closes every connection.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

fn migrator() -> Migrator {
    let migrations = MIGRATIONS
        .iter()
        .map(|&(version, description, sql)| {
            Migration::new(
                version,
                description.into(),
                MigrationType::Simple,
                sql.into_sql_str(),
                false,
            )
        })
        .collect();
    Migrator::with_migrations(migrations)
}

/// Current time as Unix seconds.
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// SQLite integers are signed; GMM's ids and sizes are not.
pub(crate) fn to_db(value: u64, what: &'static str) -> Result<i64, DbError> {
    i64::try_from(value).map_err(|_| DbError::OutOfRange(what))
}

pub(crate) fn from_db(value: i64, what: &'static str) -> Result<u64, DbError> {
    u64::try_from(value).map_err(|_| DbError::OutOfRange(what))
}

pub(crate) fn opt_to_db(value: Option<u64>, what: &'static str) -> Result<Option<i64>, DbError> {
    value.map(|v| to_db(v, what)).transpose()
}

pub(crate) fn opt_from_db(value: Option<i64>, what: &'static str) -> Result<Option<u64>, DbError> {
    value.map(|v| from_db(v, what)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refuses_a_database_from_a_newer_gmm() {
        let db = Db::open_in_memory().await.unwrap();
        // What a future GMM would leave behind after applying migration 99.
        sqlx::query(
            "INSERT INTO _sqlx_migrations
                 (version, description, success, checksum, execution_time)
             VALUES (99, 'future', 1, x'00', 0)",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        let err = migrator().run(&db.pool).await.unwrap_err();
        assert!(matches!(
            err,
            sqlx::migrate::MigrateError::VersionMissing(99)
        ));
    }
}
