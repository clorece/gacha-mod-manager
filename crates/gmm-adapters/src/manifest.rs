// SPDX-License-Identifier: MIT OR Apache-2.0

//! The manifest schema. Unknown fields are rejected so a typo can't silently disable a rule.

use std::fmt;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameManifest {
    pub game: GameInfo,
    pub gamebanana: GameBananaInfo,
    pub xxmi: Xxmi,
    pub detect: Detect,
    #[serde(rename = "mod_kind")]
    pub mod_kinds: Vec<ModKind>,
    #[serde(default)]
    pub conflicts: Conflicts,
}

impl GameManifest {
    pub fn id(&self) -> &str {
        &self.game.id
    }

    /// Arguments for `XXMI Launcher.exe` that start this game with mods, matching XXMI's own
    /// "Quick Start" shortcuts. Fixed in code, never taken from data (security rule R7).
    pub fn xxmi_launch_args(&self) -> [String; 3] {
        [
            "--nogui".to_owned(),
            "--xxmi".to_owned(),
            self.xxmi.importer.to_string(),
        ]
    }

    pub fn root_category(&self, id: u64) -> Option<&RootCategory> {
        self.gamebanana.root_categories.iter().find(|c| c.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameInfo {
    /// Short lowercase id used on the command line and in the database (`zzz`).
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub engine: Engine,
    /// Shown on the game's page.
    pub risk_notice: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Unity,
    Unreal4,
    Unreal5,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameBananaInfo {
    pub game_id: u64,
    /// The Store's top-level categories. Usually GameBanana's roots, but any category works:
    /// Endfield's characters sit under Skins > Operators, so it lists Operators.
    pub root_categories: Vec<RootCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootCategory {
    pub id: u64,
    /// GameBanana's name, for reference; the live name wins in the UI.
    pub name: String,
    pub tag: Tag,
    /// Whether subcategories are characters (shown as the "Character" filter).
    #[serde(default)]
    pub subcategories_are_characters: bool,
}

/// What a root category holds, independent of each game's naming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    CharacterSkin,
    /// Bangboo (ZZZ) and similar companions.
    CompanionSkin,
    Ui,
    Other,
}

/// XXMI model importers. Every XXMI-supported game has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Importer {
    #[serde(rename = "ZZMI")]
    Zzmi,
    #[serde(rename = "WWMI")]
    Wwmi,
    #[serde(rename = "EFMI")]
    Efmi,
    #[serde(rename = "SRMI")]
    Srmi,
    #[serde(rename = "GIMI")]
    Gimi,
    #[serde(rename = "HIMI")]
    Himi,
}

impl Importer {
    /// The id XXMI uses for folder names, config keys and `--xxmi`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zzmi => "ZZMI",
            Self::Wwmi => "WWMI",
            Self::Efmi => "EFMI",
            Self::Srmi => "SRMI",
            Self::Gimi => "GIMI",
            Self::Himi => "HIMI",
        }
    }
}

impl fmt::Display for Importer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Xxmi {
    pub importer: Importer,
    /// The process XXMI's `d3dx.ini` targets (`target = ...`). Used to spot a broken importer.
    pub d3dx_target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    /// At least one must exist directly in the game folder.
    pub exe_names: Vec<String>,
    /// All must exist, relative to the game folder.
    #[serde(default)]
    pub required_children: Vec<String>,
    /// Typical game folder names, for the user-started drive scan.
    #[serde(default)]
    pub folder_names: Vec<String>,
    pub steam: Option<SteamDetect>,
    pub epic: Option<EpicDetect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SteamDetect {
    pub appid: u32,
    /// Game folder relative to `steamapps/common/<installdir>`; empty if it's the same folder.
    #[serde(default)]
    pub subpath: String,
    /// Whether XXMI may start the Steam copy directly ("Game Launch" = Direct). False means the
    /// copy only runs when Steam starts it, so XXMI must be set to "Steam" (the setup check
    /// reports `LaunchModeMismatch` otherwise). Off by default: a wrong "off" only asks for a
    /// setting change, a wrong "on" lets the game close itself.
    #[serde(default)]
    pub direct_launch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpicDetect {
    /// Matched against `DisplayName` in Epic's install manifests.
    pub display_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModKind {
    /// `migoto` (a folder in XXMI's `Mods/`) is the only kind for now.
    pub id: String,
    /// An archive is this kind if one of its `.ini` files contains one of these.
    pub ini_markers: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflicts {
    /// Namespaces of shared library mods whose overrides are never reported as conflicts.
    #[serde(default)]
    pub ignore_namespaces: Vec<String>,
}
