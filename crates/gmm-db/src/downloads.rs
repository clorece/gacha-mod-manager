// SPDX-License-Identifier: MIT OR Apache-2.0

use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, DbError, from_db, now, opt_from_db, opt_to_db, to_db};

/// An archive kept in the download cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedDownload {
    pub sha256: String,
    pub gb_mod_id: Option<u64>,
    pub gb_file_id: Option<u64>,
    /// Path relative to the cache root, e.g. `downloads/1234-mod.zip`.
    pub path_rel: String,
    pub size_bytes: u64,
    pub added_at: i64,
    pub last_used_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewCachedDownload {
    pub sha256: String,
    pub gb_mod_id: Option<u64>,
    pub gb_file_id: Option<u64>,
    pub path_rel: String,
    pub size_bytes: u64,
}

/// `SELECT <every column> FROM download_cache <rest>`, as a static string.
macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT sha256, gb_mod_id, gb_file_id, path_rel, size_bytes, added_at, last_used_at ",
            "FROM download_cache ",
            $rest
        )
    };
}

impl Db {
    /// Records an archive in the cache, or refreshes the record if the same bytes (same
    /// SHA-256) are already there.
    pub async fn record_download(&self, d: &NewCachedDownload) -> Result<(), DbError> {
        let at = now();
        let mut tx = self.pool.begin().await?;
        // A different archive at the same path (the file changed and was downloaded again)
        // replaces the old record.
        sqlx::query("DELETE FROM download_cache WHERE path_rel = ? AND sha256 <> ?")
            .bind(&d.path_rel)
            .bind(&d.sha256)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO download_cache
                 (sha256, gb_mod_id, gb_file_id, path_rel, size_bytes, added_at, last_used_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (sha256) DO UPDATE SET
                 gb_mod_id = excluded.gb_mod_id,
                 gb_file_id = excluded.gb_file_id,
                 path_rel = excluded.path_rel,
                 size_bytes = excluded.size_bytes,
                 last_used_at = excluded.last_used_at",
        )
        .bind(&d.sha256)
        .bind(opt_to_db(d.gb_mod_id, "gb_mod_id")?)
        .bind(opt_to_db(d.gb_file_id, "gb_file_id")?)
        .bind(&d.path_rel)
        .bind(to_db(d.size_bytes, "size_bytes")?)
        .bind(at)
        .bind(at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn cached_download(&self, sha256: &str) -> Result<Option<CachedDownload>, DbError> {
        let row = sqlx::query(select!("WHERE sha256 = ?"))
            .bind(sha256)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(map_row).transpose()
    }

    /// The most recently used cached archive of a GameBanana file.
    pub async fn cached_download_for_file(
        &self,
        gb_file_id: u64,
    ) -> Result<Option<CachedDownload>, DbError> {
        let row = sqlx::query(select!(
            "WHERE gb_file_id = ?
             ORDER BY last_used_at DESC LIMIT 1"
        ))
        .bind(to_db(gb_file_id, "gb_file_id")?)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(map_row).transpose()
    }

    /// Every cached archive, least recently used first (the eviction order).
    pub async fn cached_downloads(&self) -> Result<Vec<CachedDownload>, DbError> {
        let rows = sqlx::query(select!("ORDER BY last_used_at, sha256"))
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(map_row).collect()
    }

    /// Total size of the cached archives.
    pub async fn download_cache_bytes(&self) -> Result<u64, DbError> {
        let total: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(size_bytes), 0) FROM download_cache")
                .fetch_one(&self.pool)
                .await?;
        from_db(total, "download cache size")
    }

    /// Marks an archive as just used (reinstall, rollback). Returns false if it isn't cached.
    pub async fn touch_download(&self, sha256: &str) -> Result<bool, DbError> {
        let result = sqlx::query("UPDATE download_cache SET last_used_at = ? WHERE sha256 = ?")
            .bind(now())
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Forgets an archive. The caller removes the file. Returns false if it wasn't cached.
    pub async fn remove_download(&self, sha256: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM download_cache WHERE sha256 = ?")
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn map_row(row: &SqliteRow) -> Result<CachedDownload, DbError> {
    Ok(CachedDownload {
        sha256: row.try_get("sha256")?,
        gb_mod_id: opt_from_db(row.try_get("gb_mod_id")?, "gb_mod_id")?,
        gb_file_id: opt_from_db(row.try_get("gb_file_id")?, "gb_file_id")?,
        path_rel: row.try_get("path_rel")?,
        size_bytes: from_db(row.try_get("size_bytes")?, "size_bytes")?,
        added_at: row.try_get("added_at")?,
        last_used_at: row.try_get("last_used_at")?,
    })
}
