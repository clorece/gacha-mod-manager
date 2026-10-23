// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::manifest::{GameManifest, Importer};
use crate::validate;

/// The manifests shipped with GMM. Adding a game = adding a file here and in `games/`.
const BUILTIN: &[(&str, &str)] = &[
    ("zzz.toml", include_str!("../games/zzz.toml")),
    ("wuwa.toml", include_str!("../games/wuwa.toml")),
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("{file}: could not parse: {message}")]
    Parse { file: String, message: String },
    #[error("{file}: {}", problems.join("; "))]
    Invalid { file: String, problems: Vec<String> },
    #[error("{0}")]
    Duplicate(String),
}

/// All supported games, validated.
#[derive(Debug, Clone)]
pub struct Registry {
    games: Vec<GameManifest>,
}

impl Registry {
    /// The built-in games. Parsed and validated once; the test suite guarantees this succeeds.
    pub fn builtin() -> Result<&'static Self, ManifestError> {
        static BUILTIN_REGISTRY: OnceLock<Result<Registry, ManifestError>> = OnceLock::new();
        BUILTIN_REGISTRY
            .get_or_init(|| Self::from_sources(BUILTIN))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Builds a registry from `(file name, TOML text)` pairs. Public so tests (and later,
    /// developer tooling) can check manifests that aren't built in yet.
    pub fn from_sources(sources: &[(&str, &str)]) -> Result<Self, ManifestError> {
        let mut games = Vec::with_capacity(sources.len());
        for (file, text) in sources {
            let manifest: GameManifest =
                toml::from_str(text).map_err(|e| ManifestError::Parse {
                    file: (*file).to_owned(),
                    message: e.to_string(),
                })?;
            let problems = validate::problems(&manifest);
            if !problems.is_empty() {
                return Err(ManifestError::Invalid {
                    file: (*file).to_owned(),
                    problems,
                });
            }
            games.push(manifest);
        }

        let mut ids = HashSet::new();
        let mut gb_ids = HashSet::new();
        let mut importers = HashSet::new();
        let mut appids = HashSet::new();
        for g in &games {
            if !ids.insert(g.game.id.as_str()) {
                return Err(ManifestError::Duplicate(format!(
                    "game id {:?} is used twice",
                    g.game.id
                )));
            }
            if !gb_ids.insert(g.gamebanana.game_id) {
                return Err(ManifestError::Duplicate(format!(
                    "GameBanana game id {} is used twice",
                    g.gamebanana.game_id
                )));
            }
            if !importers.insert(g.xxmi.importer) {
                return Err(ManifestError::Duplicate(format!(
                    "XXMI importer {} is used twice",
                    g.xxmi.importer
                )));
            }
            if let Some(steam) = &g.detect.steam
                && !appids.insert(steam.appid)
            {
                return Err(ManifestError::Duplicate(format!(
                    "Steam appid {} is used twice",
                    steam.appid
                )));
            }
        }
        Ok(Self { games })
    }

    pub fn games(&self) -> &[GameManifest] {
        &self.games
    }

    pub fn get(&self, id: &str) -> Option<&GameManifest> {
        self.games
            .iter()
            .find(|g| g.game.id.eq_ignore_ascii_case(id))
    }

    pub fn by_gamebanana_id(&self, id: u64) -> Option<&GameManifest> {
        self.games.iter().find(|g| g.gamebanana.game_id == id)
    }

    pub fn by_importer(&self, importer: Importer) -> Option<&GameManifest> {
        self.games.iter().find(|g| g.xxmi.importer == importer)
    }

    pub fn by_steam_appid(&self, appid: u32) -> Option<&GameManifest> {
        self.games
            .iter()
            .find(|g| g.detect.steam.as_ref().is_some_and(|s| s.appid == appid))
    }
}
