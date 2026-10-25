// SPDX-License-Identifier: MIT OR Apache-2.0

use gmm_archive::ArchiveError;
use gmm_catalog::CatalogError;
use gmm_db::DbError;
use gmm_fsguard::{FsError, PathError};
use gmm_net::NetError;

impl CoreError {
    /// For GameBanana asking GMM to slow down: how long until it may be asked again.
    pub fn retry_in(&self) -> Option<std::time::Duration> {
        match self {
            Self::Net(e) | Self::Catalog(CatalogError::Net(e)) => e.retry_in(),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Net(#[from] NetError),
    #[error(transparent)]
    Fs(#[from] FsError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("could not write gmm.json: {0}")]
    Sidecar(#[from] serde_json::Error),
    #[error("could not find the app data folders")]
    NoAppDirs,
    #[error("file error: {0}")]
    Io(String),

    #[error("{0}")]
    BadProfile(String),
    #[error("can't adopt this mod: {0}")]
    CantAdopt(String),
    #[error("can't install from this file: {0}")]
    CantInstallLocal(String),
    #[error("GameBanana mod {gb_mod_id} isn't installed for {game}")]
    NotInstalled { gb_mod_id: u64, game: String },
    #[error("{changed} folder(s) moved to the new file, then: {message}")]
    PartlyChanged { changed: usize, message: String },
    #[error("mod {mod_id} has no file {file_id}")]
    NoSuchFile { mod_id: u64, file_id: u64 },
    #[error("mod {0} has no file that GameBanana has scanned as clean")]
    NoCleanFile(u64),
    #[error(
        "file {file_id} has not been cleared by GameBanana's virus scan ({status}); refusing to \
         download it"
    )]
    NotClean { file_id: u64, status: String },
    #[error("mod {mod_id} can't be installed: {reason}")]
    NotInstallable { mod_id: u64, reason: &'static str },
    #[error("mod {mod_id} is for another game on GameBanana (game {found}, not {expected})")]
    WrongGame {
        mod_id: u64,
        expected: u64,
        found: u64,
    },
    #[error(
        "mod {mod_id} isn't for a game GMM supports (GameBanana game {})",
        .game_id.map_or("unknown".to_owned(), |id| id.to_string())
    )]
    UnsupportedGame { mod_id: u64, game_id: Option<u64> },
    #[error("{game} isn't set up for mods yet: {reason}")]
    NotReady { game: String, reason: String },

    #[error(
        "this doesn't look like an XXMI mod: no .ini file in the archive contains {markers} \
         (the archive is kept in the download cache)"
    )]
    NotAMod { markers: String },
    /// The archive holds several mods; the caller must say which to install.
    #[error("the archive holds {} variants: {}", .0.len(), .0.join(", "))]
    ChooseVariant(Vec<String>),
    #[error("no variant named {name:?} (the archive has: {})", .available.join(", "))]
    UnknownVariant {
        name: String,
        available: Vec<String>,
    },
    #[error(
        "the mod contains files that are never installed (executables or scripts): {}",
        .0.join(", ")
    )]
    Blocked(Vec<String>),
    #[error("{folder:?} is already installed for {game}; uninstall it first")]
    AlreadyInstalled { game: String, folder: String },
    #[error("{folder:?} already exists in the Mods folder but GMM didn't install it")]
    FolderTaken { folder: String },
    #[error("{0}")]
    Launch(String),
    #[error("the XXMI Launcher exe hasn't been confirmed yet: {}", .0.display())]
    ExeNotConfirmed(std::path::PathBuf),
    #[error("no mod named {query:?} in {game}'s Mods folder (see `gmm list`)")]
    NoSuchUnmanaged { query: String, game: String },
    #[error("{query:?} matches more than one mod; use the full path from `gmm list`: {}", .matches.join(", "))]
    AmbiguousUnmanaged { query: String, matches: Vec<String> },
    #[error("no mod {id} installed for {game} (see `gmm list`)")]
    NoSuchMod { id: i64, game: String },
    #[error("the folder of {0:?} is gone from the Mods folder (deleted or moved outside GMM)")]
    ModFolderMissing(String),
    #[error("both {0:?} and its DISABLED copy exist in the Mods folder; remove one by hand first")]
    ModFolderAmbiguous(String),
    /// The install was paused or cancelled ([`crate::StopSignal`]).
    #[error("{}", match .0 { crate::StopReason::Paused => "paused", crate::StopReason::Cancelled => "cancelled" })]
    Stopped(crate::StopReason),
    #[error("no installed version {id} of a GameBanana mod in {game}")]
    NotAVersion { id: i64, game: String },
    /// An image key GMM never handed out (see [`crate::ImageCache`]).
    #[error("unknown image")]
    UnknownImage,
    #[error("GameBanana didn't send an image: {0}")]
    NotAnImage(String),
}
