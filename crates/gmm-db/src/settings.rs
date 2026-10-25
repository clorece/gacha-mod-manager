// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use sqlx::Row;

use crate::{Db, DbError, now};

/// Keys of the `settings` table. Typed accessors below are the only way in, so the set of
/// things GMM remembers stays visible in one place.
#[derive(Debug, Clone, Copy)]
enum Key {
    /// An XXMI Launcher folder the user picked (portable installs that detection misses).
    XxmiRoot,
    /// The XXMI Launcher exe the user confirmed before the first launch (R7).
    ConfirmedXxmiExe,
    /// Dark, light or match Windows.
    Theme,
    /// Show mods GameBanana marks as sensitive (NSFW).
    ShowSensitive,
    /// Blur sensitive previews until hovered (only matters while they are shown).
    BlurSensitive,
    /// The first-run preferences screen was answered.
    Onboarded,
}

impl Key {
    fn as_str(self) -> &'static str {
        match self {
            Self::XxmiRoot => "xxmi.root",
            Self::ConfirmedXxmiExe => "xxmi.confirmed_exe",
            Self::Theme => "ui.theme",
            Self::ShowSensitive => "ui.show_sensitive",
            Self::BlurSensitive => "ui.blur_sensitive",
            Self::Onboarded => "ui.onboarded",
        }
    }
}

/// The app's look.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    /// Follow Windows' light or dark setting.
    System,
}

impl Theme {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

/// What the user chose on the first-run screen, in Settings or in the Store and Library
/// filters (one setting, shown in all three places).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preferences {
    pub theme: Theme,
    pub show_sensitive: bool,
    pub blur_sensitive: bool,
    pub onboarded: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            show_sensitive: false,
            blur_sensitive: true,
            onboarded: false,
        }
    }
}

/// A game folder the user named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualGameFolder {
    pub game_id: String,
    pub folder: PathBuf,
    pub added_at: i64,
}

impl Db {
    async fn get_setting(&self, key: Key) -> Result<Option<String>, DbError> {
        Ok(
            sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
                .bind(key.as_str())
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn set_setting(&self, key: Key, value: Option<&str>) -> Result<(), DbError> {
        match value {
            Some(value) => {
                sqlx::query(
                    "INSERT INTO settings (key, value) VALUES (?, ?)
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                )
                .bind(key.as_str())
                .bind(value)
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM settings WHERE key = ?")
                    .bind(key.as_str())
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    async fn get_path(&self, key: Key) -> Result<Option<PathBuf>, DbError> {
        Ok(self.get_setting(key).await?.map(PathBuf::from))
    }

    async fn set_path(&self, key: Key, path: Option<&Path>) -> Result<(), DbError> {
        let value = path.map(path_str).transpose()?;
        self.set_setting(key, value).await
    }

    /// The XXMI Launcher folder the user picked, if any.
    pub async fn xxmi_root(&self) -> Result<Option<PathBuf>, DbError> {
        self.get_path(Key::XxmiRoot).await
    }

    pub async fn set_xxmi_root(&self, root: Option<&Path>) -> Result<(), DbError> {
        self.set_path(Key::XxmiRoot, root).await
    }

    /// The XXMI Launcher exe the user confirmed, if any. `gmm launch` only runs this exact
    /// path, and only while it is still inside the detected XXMI root.
    pub async fn confirmed_xxmi_exe(&self) -> Result<Option<PathBuf>, DbError> {
        self.get_path(Key::ConfirmedXxmiExe).await
    }

    pub async fn set_confirmed_xxmi_exe(&self, exe: Option<&Path>) -> Result<(), DbError> {
        self.set_path(Key::ConfirmedXxmiExe, exe).await
    }

    /// The saved preferences; anything never set (or unreadable) has its default.
    pub async fn preferences(&self) -> Result<Preferences, DbError> {
        let d = Preferences::default();
        let flag = |v: Option<String>, default: bool| v.map_or(default, |v| v == "1");
        Ok(Preferences {
            theme: self
                .get_setting(Key::Theme)
                .await?
                .and_then(|v| Theme::parse(&v))
                .unwrap_or(d.theme),
            show_sensitive: flag(
                self.get_setting(Key::ShowSensitive).await?,
                d.show_sensitive,
            ),
            blur_sensitive: flag(
                self.get_setting(Key::BlurSensitive).await?,
                d.blur_sensitive,
            ),
            onboarded: flag(self.get_setting(Key::Onboarded).await?, d.onboarded),
        })
    }

    pub async fn set_preferences(&self, p: &Preferences) -> Result<(), DbError> {
        let flag = |b: bool| if b { "1" } else { "0" };
        self.set_setting(Key::Theme, Some(p.theme.as_str())).await?;
        self.set_setting(Key::ShowSensitive, Some(flag(p.show_sensitive)))
            .await?;
        self.set_setting(Key::BlurSensitive, Some(flag(p.blur_sensitive)))
            .await?;
        self.set_setting(Key::Onboarded, Some(flag(p.onboarded)))
            .await
    }

    /// Game folders the user named, oldest first.
    pub async fn manual_game_folders(&self) -> Result<Vec<ManualGameFolder>, DbError> {
        let rows = sqlx::query(
            "SELECT game_id, folder, added_at FROM manual_game_folders
             ORDER BY added_at, game_id, folder",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(ManualGameFolder {
                    game_id: row.try_get("game_id")?,
                    folder: PathBuf::from(row.try_get::<String, _>("folder")?),
                    added_at: row.try_get("added_at")?,
                })
            })
            .collect()
    }

    /// Remembers a game folder. Returns false if it was already stored.
    pub async fn add_manual_game_folder(
        &self,
        game_id: &str,
        folder: &Path,
    ) -> Result<bool, DbError> {
        let result = sqlx::query(
            "INSERT INTO manual_game_folders (game_id, folder, added_at) VALUES (?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(game_id)
        .bind(path_str(folder)?)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Forgets a game folder. Windows paths compare case-insensitively, so the match ignores
    /// ASCII case there. Returns false if nothing matched.
    pub async fn remove_manual_game_folder(
        &self,
        game_id: &str,
        folder: &Path,
    ) -> Result<bool, DbError> {
        let sql = if cfg!(windows) {
            "DELETE FROM manual_game_folders WHERE game_id = ? AND folder = ? COLLATE NOCASE"
        } else {
            "DELETE FROM manual_game_folders WHERE game_id = ? AND folder = ?"
        };
        let result = sqlx::query(sql)
            .bind(game_id)
            .bind(path_str(folder)?)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Records that GMM's Launch button started `game_id` now.
    pub async fn record_played(&self, game_id: &str) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO game_activity (game_id, last_played) VALUES (?, ?)
             ON CONFLICT (game_id) DO UPDATE SET last_played = excluded.last_played",
        )
        .bind(game_id)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// When GMM last started each game (Unix seconds), by game id.
    pub async fn last_played(&self) -> Result<Vec<(String, i64)>, DbError> {
        let rows = sqlx::query("SELECT game_id, last_played FROM game_activity ORDER BY game_id")
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|row| Ok((row.try_get("game_id")?, row.try_get("last_played")?)))
            .collect()
    }
}

fn path_str(path: &Path) -> Result<&str, DbError> {
    path.to_str()
        .ok_or_else(|| DbError::NonUtf8Path(path.to_path_buf()))
}
