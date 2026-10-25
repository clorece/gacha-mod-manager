// SPDX-License-Identifier: MIT OR Apache-2.0

use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, DbError, now};

/// A saved profile. `entries_json` belongs to gmm-core, which reads and writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRow {
    pub id: i64,
    pub game_id: String,
    pub name: String,
    pub entries_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Db {
    /// A game's profiles, by name.
    pub async fn profiles(&self, game_id: &str) -> Result<Vec<ProfileRow>, DbError> {
        let rows = sqlx::query(
            "SELECT id, game_id, name, entries_json, created_at, updated_at FROM profiles
             WHERE game_id = ? ORDER BY name COLLATE NOCASE, id",
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_row).collect()
    }

    pub async fn profile(&self, id: i64) -> Result<Option<ProfileRow>, DbError> {
        let row = sqlx::query(
            "SELECT id, game_id, name, entries_json, created_at, updated_at FROM profiles
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(map_row).transpose()
    }

    /// Saves a profile under `name`, replacing the entries of one with that name. Returns
    /// its id.
    pub async fn save_profile(
        &self,
        game_id: &str,
        name: &str,
        entries_json: &str,
    ) -> Result<i64, DbError> {
        let at = now();
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO profiles (game_id, name, entries_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (game_id, name) DO UPDATE
                 SET entries_json = excluded.entries_json, updated_at = excluded.updated_at
             RETURNING id",
        )
        .bind(game_id)
        .bind(name)
        .bind(entries_json)
        .bind(at)
        .bind(at)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Returns false if there is no such profile, or one with the new name exists.
    pub async fn rename_profile(&self, id: i64, name: &str) -> Result<bool, DbError> {
        let result =
            sqlx::query("UPDATE OR IGNORE profiles SET name = ?, updated_at = ? WHERE id = ?")
                .bind(name)
                .bind(now())
                .bind(id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Returns false if there is no such profile.
    pub async fn delete_profile(&self, id: i64) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM profiles WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn map_row(row: &SqliteRow) -> Result<ProfileRow, DbError> {
    Ok(ProfileRow {
        id: row.try_get("id")?,
        game_id: row.try_get("game_id")?,
        name: row.try_get("name")?,
        entries_json: row.try_get("entries_json")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
