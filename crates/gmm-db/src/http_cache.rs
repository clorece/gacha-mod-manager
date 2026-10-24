// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use gmm_catalog::{CacheFuture, HttpCache};

use crate::{Db, DbError, from_db, now};

fn secs(d: Duration) -> i64 {
    i64::try_from(d.as_secs()).unwrap_or(i64::MAX)
}

impl Db {
    /// A cached response body for `url`, if one was stored within `max_age`.
    pub async fn http_cache_get(
        &self,
        url: &str,
        max_age: Duration,
    ) -> Result<Option<Vec<u8>>, DbError> {
        let oldest = now().saturating_sub(secs(max_age));
        Ok(
            sqlx::query_scalar("SELECT body FROM http_cache WHERE url = ? AND fetched_at >= ?")
                .bind(url)
                .bind(oldest)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn http_cache_put(&self, url: &str, body: &[u8]) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO http_cache (url, body, fetched_at) VALUES (?, ?, ?)
             ON CONFLICT (url) DO UPDATE SET body = excluded.body, fetched_at = excluded.fetched_at",
        )
        .bind(url)
        .bind(body)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deletes responses older than `keep`, then all but the newest `max_rows`. Returns how
    /// many were deleted.
    pub async fn prune_http_cache(&self, keep: Duration, max_rows: i64) -> Result<u64, DbError> {
        let old = sqlx::query("DELETE FROM http_cache WHERE fetched_at < ?")
            .bind(now().saturating_sub(secs(keep)))
            .execute(&self.pool)
            .await?;
        let excess = sqlx::query(
            "DELETE FROM http_cache WHERE url NOT IN
                 (SELECT url FROM http_cache ORDER BY fetched_at DESC, url LIMIT ?)",
        )
        .bind(max_rows.max(0))
        .execute(&self.pool)
        .await?;
        Ok(old.rows_affected() + excess.rows_affected())
    }

    /// Removes every cached response (Storage screen, "clear cache").
    pub async fn clear_http_cache(&self) -> Result<u64, DbError> {
        let result = sqlx::query("DELETE FROM http_cache")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Number of cached responses and their total size.
    pub async fn http_cache_stats(&self) -> Result<(u64, u64), DbError> {
        let (count, bytes): (i64, i64) =
            sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(LENGTH(body)), 0) FROM http_cache")
                .fetch_one(&self.pool)
                .await?;
        Ok((
            from_db(count, "http cache rows")?,
            from_db(bytes, "http cache size")?,
        ))
    }
}

/// The catalogue's response cache, kept in the database. Errors count as a miss: a broken
/// cache must never stop browsing.
impl HttpCache for Db {
    fn get<'a>(&'a self, key: &'a str, max_age: Duration) -> CacheFuture<'a, Option<Vec<u8>>> {
        Box::pin(async move { self.http_cache_get(key, max_age).await.ok().flatten() })
    }

    fn put<'a>(&'a self, key: &'a str, body: Vec<u8>) -> CacheFuture<'a, ()> {
        Box::pin(async move {
            let _ = self.http_cache_put(key, &body).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn insert_at(db: &Db, url: &str, fetched_at: i64) {
        sqlx::query("INSERT INTO http_cache (url, body, fetched_at) VALUES (?, x'00', ?)")
            .bind(url)
            .bind(fetched_at)
            .execute(&db.pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn stale_entries_miss_and_get_pruned() {
        let db = Db::open_in_memory().await.unwrap();
        let t = now();
        insert_at(&db, "old", t - 10 * 24 * 3600).await;
        insert_at(&db, "hour", t - 3600 - 5).await;
        insert_at(&db, "new", t).await;

        let minute = Duration::from_secs(60);
        assert_eq!(db.http_cache_get("hour", minute).await.unwrap(), None);
        assert!(
            db.http_cache_get("hour", Duration::from_secs(2 * 3600))
                .await
                .unwrap()
                .is_some()
        );

        // Older than a week: gone. Then the row cap keeps only the newest.
        let week = Duration::from_secs(7 * 24 * 3600);
        assert_eq!(db.prune_http_cache(week, 10).await.unwrap(), 1);
        assert_eq!(db.prune_http_cache(week, 1).await.unwrap(), 1);
        assert!(db.http_cache_get("new", minute).await.unwrap().is_some());
        assert_eq!(db.http_cache_stats().await.unwrap().0, 1);
    }
}
