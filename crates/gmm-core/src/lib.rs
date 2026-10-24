// SPDX-License-Identifier: MIT OR Apache-2.0

//! GMM's features, built from the lower crates. The CLI and (later) the app's commands are
//! thin layers over this crate.
//!
//! Installing a mod (docs/02-architecture.md, "Install flow"):
//!
//! 1. [`fetch_archive`]: pick a file GameBanana scanned as clean, download it into the cache
//!    (or reuse the cached copy), record it in the database.
//! 2. [`analyze`]: extract it into a fresh folder under the cache, run the policy scan, and
//!    work out what kind of mod it is and which variants it holds.
//! 3. [`install_analyzed`]: copy the chosen variant(s) into a staging folder under
//!    `<XXMI>/<IMP>/Mods/GMM/`, write each mod's `gmm.json`, then rename each one into place
//!    and record it in the database.
//!
//! [`install`] runs all three and removes the extraction folder afterwards. [`install_gate`]
//! decides from a game's setup state whether installing makes sense yet.

mod analyze;
mod conflicts;
mod error;
mod fetch;
mod install;
mod library;
mod names;
mod storage;
mod workspace;

pub use analyze::{Analysis, Variant, analyze};
pub use conflicts::{Conflict, ConflictReport, ModRef, conflicts, conflicts_for};
pub use error::CoreError;
pub use fetch::{FetchEvent, FetchedArchive, MAX_DOWNLOAD_BYTES, choose_file, fetch_archive};
pub use install::{
    InstallEvent, InstallOptions, InstallReport, InstallRequest, InstalledFolder, ModMeta, Sidecar,
    VariantChoice, install, install_analyzed, install_gate,
};
pub use library::{
    DiskState, Library, ManagedMod, RecycleBin, ToggleOutcome, Trash, UninstallOutcome,
    UnmanagedMod, library, set_enabled, uninstall,
};
pub use names::{
    DISABLED_PREFIX, GMM_DIR, SIDECAR_FILE, STAGING_DIR, mod_folder_name, on_disk_name,
};
pub use storage::{
    CacheStorage, ClearedCache, GameStorage, ModUsage, Size, cache_storage, clear_downloads,
    game_storage,
};
pub use workspace::{AppDirs, Workspace};
