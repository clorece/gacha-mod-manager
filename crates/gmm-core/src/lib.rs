// SPDX-License-Identifier: MIT OR Apache-2.0

//! GMM's features, built from the lower crates. The CLI and the app's commands are thin
//! layers over this crate.
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
//! [`install`] runs all three and removes the extraction folder afterwards. The app splits it
//! in two for its confirmation dialog: [`prepare`] (steps 1 and 2, then [`install_plan`] for
//! what the dialog shows) and [`install_prepared`]. [`install_gate`] decides from a game's
//! setup state whether installing makes sense yet. A [`StopSignal`] pauses or cancels an
//! install; [`InstallQueue`] keeps the Downloads page's list.

mod adopt;
mod analyze;
mod conflicts;
mod errlog;
mod error;
mod fetch;
mod images;
mod install;
mod keys;
mod launch;
mod library;
mod local;
mod names;
mod plan;
mod profiles;
mod queue;
mod report;
mod setup;
mod stop;
mod storage;
mod store;
mod toggles;
mod update;
mod updates;
mod versions;
mod workspace;

pub use adopt::{AdoptCandidate, Adopted, MatchReason, adopt, adopt_candidates};
pub use analyze::{Analysis, Variant, VersionConflict, analyze};
pub use conflicts::{
    Conflict, ConflictReport, ModRef, conflicts, conflicts_after_install, conflicts_for,
    conflicts_for_unmanaged,
};
pub use errlog::{ErrorLog, LogEntry, MAX_LOG_BYTES, utc};
pub use error::CoreError;
pub use fetch::{FetchEvent, FetchedArchive, MAX_DOWNLOAD_BYTES, choose_file, fetch_archive};
pub use images::{CachedImage, IMAGE_HOST, ImageCache, MAX_IMAGE_CACHE_BYTES, clear_image_cache};
pub use install::{
    InstallEvent, InstallOptions, InstallReport, InstallRequest, InstalledFolder, ModMeta,
    Prepared, Sidecar, VariantChoice, install, install_analyzed, install_gate, install_prepared,
    prepare,
};
pub use keys::{KeyBindings, ModKeyBinding, mod_keybindings};
pub use launch::{LaunchPlan, confirm_exe, plan_launch, start};
pub use library::{
    DeleteForever, DiskState, Library, ManagedMod, ModFolder, Remover, ToggleOutcome,
    UninstallOutcome, UnmanagedMod, UnmanagedToggle, find_unmanaged, library, listed_unmanaged,
    mod_folder, one_unmanaged, set_enabled, set_unmanaged_enabled, set_unmanaged_enabled_at,
    uninstall, uninstall_unmanaged,
};
pub use local::{
    LOCAL_ARCHIVE_TYPES, LocalPlan, LocalPrepared, install_local, local_plan, prepare_local,
};
pub use names::{
    DISABLED_PREFIX, GMM_DIR, SIDECAR_FILE, STAGING_DIR, mod_folder_name, on_disk_name,
};
pub use plan::{InstallPlan, VersionPlan, install_plan};
pub use profiles::{
    MAX_PROFILE_NAME, MAX_SHARE_CODE, ProfileApplied, ProfileImported, ProfileInfo, SharedMod,
    apply_profile, delete_profile, export_profile, import_profile, profiles, rename_profile,
    save_profile,
};
pub use queue::{InstallQueue, Job, JobAction, JobId, JobLabel, JobRequest, JobState, NextJob};
pub use report::{BugReport, ISSUES_URL, MAX_TITLE_CHARS, bug_report, system_details};
pub use setup::{Machine, ModsTarget, SetupHints, game_for_mod, saved_setup};
pub use stop::{StopReason, StopSignal};
pub use storage::{
    CacheStorage, ClearedCache, GameStorage, ModUsage, Size, cache_storage, clear_downloads,
    game_storage, unmanaged_size,
};
pub use store::{Browse, FEATURED_COUNT, FeaturedMod, MAX_BROWSE_STREAMS, browse, featured};
pub use toggles::{USER_INI, restore_pending};
pub use update::{ChangedFolder, FileChangeReport, change_file, rollback_choices};
pub use updates::{ModUpdateInfo, UpdateCheck, check_updates, refresh_catalog};
pub use versions::{switch_version, version_group};
pub use workspace::{AppDirs, Workspace, open_db};
