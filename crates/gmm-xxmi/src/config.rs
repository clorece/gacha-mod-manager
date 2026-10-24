// SPDX-License-Identifier: MIT OR Apache-2.0

//! `XXMI Launcher Config.json`, reduced to the allowlisted fields.
//!
//! serde skips every field these structs don't declare without storing it, so secrets in the
//! file (`Launcher.github_token`, `Launcher.proxy`, `Security.user_signature`, `*_signature`)
//! never reach GMM's memory as values, logs, caches or output.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct XxmiConfig {
    /// The launcher version that last wrote the file (observed: matches the launcher version).
    pub config_version: Option<String>,
    /// Keyed by importer id (`ZZMI`, `WWMI`, ...).
    pub importers: BTreeMap<String, ImporterConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImporterConfig {
    pub game_folder: Option<String>,
    pub importer_folder: Option<String>,
    /// XXMI's "Game Launch" setting: `DIRECT`, `STEAM`, `EPIC_GAMES`, `CUSTOM`, `MANUAL`.
    pub game_launch: Option<String>,
    /// How many times XXMI has started the game.
    pub launch_count: Option<i64>,
}

impl XxmiConfig {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let raw: RawConfig = serde_json::from_slice(bytes).map_err(|e| {
            // serde_json's message names the position, never the content.
            format!("XXMI config is not valid JSON ({e})")
        })?;
        Ok(Self {
            config_version: raw.launcher.config_version.and_then(non_empty),
            importers: raw
                .importers
                .into_iter()
                .map(|(name, section)| {
                    let i = section.importer;
                    (
                        name,
                        ImporterConfig {
                            game_folder: i.game_folder.and_then(non_empty),
                            importer_folder: i.importer_folder.and_then(non_empty),
                            game_launch: i.game_launch.and_then(non_empty),
                            launch_count: i.launch_count,
                        },
                    )
                })
                .collect(),
        })
    }

    pub fn importer(&self, id: &str) -> Option<&ImporterConfig> {
        self.importers.get(id)
    }
}

fn non_empty(s: String) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

// Raw shapes: only what we declare is kept. No `deny_unknown_fields`: the file has many
// fields we deliberately ignore.

#[derive(Deserialize, Default)]
struct RawConfig {
    #[serde(rename = "Launcher", default)]
    launcher: RawLauncher,
    #[serde(rename = "Importers", default)]
    importers: BTreeMap<String, RawImporterSection>,
}

#[derive(Deserialize, Default)]
struct RawLauncher {
    #[serde(default)]
    config_version: Option<String>,
}

#[derive(Deserialize, Default)]
struct RawImporterSection {
    #[serde(rename = "Importer", default)]
    importer: RawImporter,
}

#[derive(Deserialize, Default)]
struct RawImporter {
    #[serde(default)]
    game_folder: Option<String>,
    #[serde(default)]
    importer_folder: Option<String>,
    #[serde(default)]
    game_launch: Option<String>,
    #[serde(default)]
    launch_count: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_declared_fields() {
        let json = br#"{
            "Launcher": { "config_version": "2.4.9", "github_token": "ghp_CANARY_TOKEN",
                          "proxy": { "user": "u", "password": "CANARY_PROXY_PW" } },
            "Importers": {
                "ZZMI": { "Importer": { "game_folder": "D:\\HoYoverse\\ZenlessZoneZero Game",
                                        "game_launch": "DIRECT", "launch_count": 3,
                                        "custom_launch_signature": "CANARY_SIG" },
                          "Migoto": { "anything": 1 } },
                "WWMI": { "Importer": { "game_folder": "", "game_launch": "STEAM" } }
            },
            "Security": { "user_signature": "CANARY_USER_SIG" }
        }"#;
        let config = XxmiConfig::parse(json).unwrap();
        assert_eq!(config.config_version.as_deref(), Some("2.4.9"));
        let zzmi = config.importer("ZZMI").unwrap();
        assert_eq!(zzmi.game_launch.as_deref(), Some("DIRECT"));
        assert_eq!(zzmi.launch_count, Some(3));
        assert_eq!(
            config.importer("WWMI").unwrap().game_folder,
            None,
            "empty is None"
        );
        let everything = format!("{config:?} {}", serde_json::to_string(&config).unwrap());
        assert!(!everything.contains("CANARY"), "{everything}");
    }

    #[test]
    fn bad_json_message_does_not_echo_content() {
        let err = XxmiConfig::parse(br#"{"Launcher": {"github_token": "ghp_SECRET" "#).unwrap_err();
        assert!(!err.contains("ghp_SECRET"), "{err}");
    }
}
