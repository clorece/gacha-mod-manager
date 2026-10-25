// SPDX-License-Identifier: MIT OR Apache-2.0

use sqlx::Row;

use crate::{Db, DbError, now};

/// The saved in-game toggles of one mod (see migration 0004). `lines` belongs to gmm-core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedToggles {
    pub game_id: String,
    pub mod_path: String,
    pub lines: String,
    pub pending: bool,
    pub saved_at: i64,
}

impl Db {
    /// Saves a mod's toggle lines, replacing what was saved before. Clears `pending`.
    pub async fn save_toggles(
        &self,
        game_id: &str,
        mod_path: &str,
        lines: &str,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO saved_toggles (game_id, mod_path, lines, pending, saved_at)
             VALUES (?, ?, ?, 0, ?)
             ON CONFLICT (game_id, mod_path) DO UPDATE
                 SET lines = excluded.lines, pending = 0, saved_at = excluded.saved_at",
        )
        .bind(game_id)
        .bind(mod_path)
        .bind(lines)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn saved_toggles(
        &self,
        game_id: &str,
        mod_path: &str,
    ) -> Result<Option<SavedToggles>, DbError> {
        let row = sqlx::query(
            "SELECT game_id, mod_path, lines, pending, saved_at FROM saved_toggles
             WHERE game_id = ? AND mod_path = ?",
        )
        .bind(game_id)
        .bind(mod_path)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(map_row).transpose()
    }

    /// The mods of a game whose write-back waits for the game to close.
    pub async fn pending_toggles(&self, game_id: &str) -> Result<Vec<SavedToggles>, DbError> {
        let rows = sqlx::query(
            "SELECT game_id, mod_path, lines, pending, saved_at FROM saved_toggles
             WHERE game_id = ? AND pending = 1 ORDER BY mod_path",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_row).collect()
    }

    /// Marks (or clears) a waiting write-back. Returns false if nothing is saved for the mod.
    pub async fn set_toggles_pending(
        &self,
        game_id: &str,
        mod_path: &str,
        pending: bool,
    ) -> Result<bool, DbError> {
        let result =
            sqlx::query("UPDATE saved_toggles SET pending = ? WHERE game_id = ? AND mod_path = ?")
                .bind(i64::from(pending))
                .bind(game_id)
                .bind(mod_path)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Forgets a mod's saved toggles (it was uninstalled). Returns false if none were saved.
    pub async fn forget_toggles(&self, game_id: &str, mod_path: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM saved_toggles WHERE game_id = ? AND mod_path = ?")
            .bind(game_id)
            .bind(mod_path)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> Result<SavedToggles, DbError> {
    Ok(SavedToggles {
        game_id: row.try_get("game_id")?,
        mod_path: row.try_get("mod_path")?,
        lines: row.try_get("lines")?,
        pending: row.try_get::<i64, _>("pending")? != 0,
        saved_at: row.try_get("saved_at")?,
    })
}

#[cfg(test)]
mod tests {
    use crate::Db;

    #[tokio::test]
    async fn saved_toggles_round_trip() {
        let db = Db::open_in_memory().await.unwrap();
        let path = "gmm/ellen [gb-1]";
        assert_eq!(db.saved_toggles("zzz", path).await.unwrap(), None);
        db.save_toggles("zzz", path, "$\\mods\\gmm\\ellen [gb-1]\\mod.ini\\swap = 2")
            .await
            .unwrap();
        assert!(db.set_toggles_pending("zzz", path, true).await.unwrap());
        assert_eq!(db.pending_toggles("zzz").await.unwrap().len(), 1);
        assert!(db.pending_toggles("wuwa").await.unwrap().is_empty());

        db.save_toggles("zzz", path, "$\\mods\\gmm\\ellen [gb-1]\\mod.ini\\swap = 3")
            .await
            .unwrap();
        let saved = db.saved_toggles("zzz", path).await.unwrap().unwrap();
        assert!(saved.lines.ends_with("= 3") && !saved.pending);

        assert!(db.forget_toggles("zzz", path).await.unwrap());
        assert!(!db.set_toggles_pending("zzz", path, true).await.unwrap());
    }
}
