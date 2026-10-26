// SPDX-License-Identifier: MIT OR Apache-2.0

use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, DbError, from_db, now, opt_from_db, opt_to_db, to_db};

/// A mod GMM installed under `<XXMI>/<IMP>/Mods/GMM/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledMod {
    pub id: i64,
    pub game_id: String,
    /// The folder name without the `DISABLED ` prefix. Chosen once at install and never
    /// changed, because 3DMigoto keys saved toggle state by path.
    pub folder_name: String,
    pub name: String,
    pub gb_mod_id: Option<u64>,
    pub gb_file_id: Option<u64>,
    pub variant: Option<String>,
    pub version: Option<String>,
    /// GameBanana's upload date of the installed file (Unix seconds).
    pub file_date: Option<i64>,
    pub enabled: bool,
    pub archive_sha256: Option<String>,
    pub size_bytes: u64,
    pub installed_at: i64,
    pub scan_report_json: Option<String>,
    pub listing: ModListing,
}

/// What GameBanana's listing said about the mod when it was installed: categories, the
/// sensitive flag and the first preview image (empty for mods installed before GMM recorded
/// them, and for hand-installed ones).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModListing {
    pub root_category_id: Option<u64>,
    /// e.g. "Character Skins".
    pub root_category: Option<String>,
    pub category_id: Option<u64>,
    /// The subcategory: for skins, the character.
    pub category: Option<String>,
    /// GameBanana flagged it (visibility or content ratings); `None` when unknown.
    pub sensitive: Option<bool>,
    /// The first preview image on GameBanana's image host.
    pub preview_url: Option<String>,
}

/// What the install pipeline records for a new mod. It starts enabled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewInstalledMod {
    pub game_id: String,
    pub folder_name: String,
    pub name: String,
    pub gb_mod_id: Option<u64>,
    pub gb_file_id: Option<u64>,
    pub variant: Option<String>,
    pub version: Option<String>,
    pub file_date: Option<i64>,
    pub archive_sha256: Option<String>,
    pub size_bytes: u64,
    pub scan_report_json: Option<String>,
    pub listing: ModListing,
}

/// What changes when an installed mod moves to another GameBanana file (an update or a
/// roll back), including a fresh listing from the mod page. The folder name, on/off state
/// and install date stay.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileChange {
    pub gb_file_id: Option<u64>,
    pub version: Option<String>,
    pub file_date: Option<i64>,
    pub archive_sha256: Option<String>,
    pub size_bytes: u64,
    pub scan_report_json: Option<String>,
    pub listing: ModListing,
}

/// `SELECT <every column> FROM installed_mods <rest>`, as a static string (sqlx only takes
/// static SQL, which rules out injection by construction).
macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT id, game_id, folder_name, name, gb_mod_id, gb_file_id, variant, version, ",
            "file_date, enabled, archive_sha256, size_bytes, installed_at, scan_report_json, ",
            "root_category_id, root_category, category_id, category, sensitive, preview_url ",
            "FROM installed_mods ",
            $rest
        )
    };
}

impl Db {
    /// Records a newly installed mod and returns its id. Fails if the game already has a mod
    /// with that folder name.
    pub async fn insert_installed_mod(&self, m: &NewInstalledMod) -> Result<i64, DbError> {
        let result = sqlx::query(
            "INSERT INTO installed_mods (game_id, folder_name, name, gb_mod_id, gb_file_id,
                 variant, version, file_date, enabled, archive_sha256, size_bytes,
                 installed_at, scan_report_json, root_category_id, root_category, category_id,
                 category, sensitive, preview_url)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&m.game_id)
        .bind(&m.folder_name)
        .bind(&m.name)
        .bind(opt_to_db(m.gb_mod_id, "gb_mod_id")?)
        .bind(opt_to_db(m.gb_file_id, "gb_file_id")?)
        .bind(&m.variant)
        .bind(&m.version)
        .bind(m.file_date)
        .bind(&m.archive_sha256)
        .bind(to_db(m.size_bytes, "size_bytes")?)
        .bind(now())
        .bind(&m.scan_report_json)
        .bind(opt_to_db(m.listing.root_category_id, "root_category_id")?)
        .bind(&m.listing.root_category)
        .bind(opt_to_db(m.listing.category_id, "category_id")?)
        .bind(&m.listing.category)
        .bind(m.listing.sensitive)
        .bind(&m.listing.preview_url)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn installed_mod(&self, id: i64) -> Result<Option<InstalledMod>, DbError> {
        let row = sqlx::query(select!("WHERE id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(map_row).transpose()
    }

    pub async fn installed_mod_by_folder(
        &self,
        game_id: &str,
        folder_name: &str,
    ) -> Result<Option<InstalledMod>, DbError> {
        let row = sqlx::query(select!("WHERE game_id = ? AND folder_name = ?"))
            .bind(game_id)
            .bind(folder_name)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(map_row).transpose()
    }

    /// Installed mods, for one game or all, sorted by game then name.
    pub async fn installed_mods(
        &self,
        game_id: Option<&str>,
    ) -> Result<Vec<InstalledMod>, DbError> {
        let rows = sqlx::query(select!(
            "WHERE ?1 IS NULL OR game_id = ?1
             ORDER BY game_id, name COLLATE NOCASE, id"
        ))
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_row).collect()
    }

    /// Every installed copy of a GameBanana mod (one per variant).
    pub async fn installed_mods_for_gb(
        &self,
        gb_mod_id: u64,
    ) -> Result<Vec<InstalledMod>, DbError> {
        let rows = sqlx::query(select!("WHERE gb_mod_id = ? ORDER BY id"))
            .bind(to_db(gb_mod_id, "gb_mod_id")?)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(map_row).collect()
    }

    /// Returns false if there is no such mod.
    pub async fn set_mod_enabled(&self, id: i64, enabled: bool) -> Result<bool, DbError> {
        let result = sqlx::query("UPDATE installed_mods SET enabled = ? WHERE id = ?")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Returns false if there is no such mod.
    pub async fn set_mod_size(&self, id: i64, size_bytes: u64) -> Result<bool, DbError> {
        let result = sqlx::query("UPDATE installed_mods SET size_bytes = ? WHERE id = ?")
            .bind(to_db(size_bytes, "size_bytes")?)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Points a mod at another file of its GameBanana mod. Returns false if there is no such
    /// mod.
    pub async fn change_installed_mod_file(
        &self,
        id: i64,
        change: &FileChange,
    ) -> Result<bool, DbError> {
        let result = sqlx::query(
            "UPDATE installed_mods SET gb_file_id = ?, version = ?, file_date = ?,
                 archive_sha256 = ?, size_bytes = ?, scan_report_json = ?,
                 root_category_id = ?, root_category = ?, category_id = ?, category = ?,
                 sensitive = ?, preview_url = ?
             WHERE id = ?",
        )
        .bind(opt_to_db(change.gb_file_id, "gb_file_id")?)
        .bind(&change.version)
        .bind(change.file_date)
        .bind(&change.archive_sha256)
        .bind(to_db(change.size_bytes, "size_bytes")?)
        .bind(&change.scan_report_json)
        .bind(opt_to_db(
            change.listing.root_category_id,
            "root_category_id",
        )?)
        .bind(&change.listing.root_category)
        .bind(opt_to_db(change.listing.category_id, "category_id")?)
        .bind(&change.listing.category)
        .bind(change.listing.sensitive)
        .bind(&change.listing.preview_url)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Returns false if there is no such mod.
    pub async fn delete_installed_mod(&self, id: i64) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM installed_mods WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn map_row(row: &SqliteRow) -> Result<InstalledMod, DbError> {
    Ok(InstalledMod {
        id: row.try_get("id")?,
        game_id: row.try_get("game_id")?,
        folder_name: row.try_get("folder_name")?,
        name: row.try_get("name")?,
        gb_mod_id: opt_from_db(row.try_get("gb_mod_id")?, "gb_mod_id")?,
        gb_file_id: opt_from_db(row.try_get("gb_file_id")?, "gb_file_id")?,
        variant: row.try_get("variant")?,
        version: row.try_get("version")?,
        file_date: row.try_get("file_date")?,
        enabled: row.try_get("enabled")?,
        archive_sha256: row.try_get("archive_sha256")?,
        size_bytes: from_db(row.try_get("size_bytes")?, "size_bytes")?,
        installed_at: row.try_get("installed_at")?,
        scan_report_json: row.try_get("scan_report_json")?,
        listing: ModListing {
            root_category_id: opt_from_db(row.try_get("root_category_id")?, "root_category_id")?,
            root_category: row.try_get("root_category")?,
            category_id: opt_from_db(row.try_get("category_id")?, "category_id")?,
            category: row.try_get("category")?,
            sensitive: row.try_get("sensitive")?,
            preview_url: row.try_get("preview_url")?,
        },
    })
}
