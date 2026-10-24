// SPDX-License-Identifier: MIT OR Apache-2.0

use gmm_archive::ArchiveError;
use gmm_catalog::CatalogError;
use gmm_db::DbError;
use gmm_fsguard::{FsError, PathError};
use gmm_net::NetError;

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
    #[error("can't install for {game} yet: {reason}")]
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
}
