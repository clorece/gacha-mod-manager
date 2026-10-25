// SPDX-License-Identifier: MIT OR Apache-2.0

//! `apps/desktop/src/bindings.ts`: the TypeScript types of every command's arguments and
//! results, generated from the Rust types so the two can't drift. The test fails when the
//! file is out of date; `GMM_UPDATE_BINDINGS=1 cargo test -p gmm-desktop bindings` rewrites
//! it.

// Tests may read and write the bindings file directly.
#![allow(clippy::disallowed_methods)]

use std::path::Path;

use ts_rs::{Config, TS};

use crate::error::CmdError;
use crate::views::*;

macro_rules! declarations {
    ($cfg:expr; $($ty:ty),* $(,)?) => {
        vec![$(format!("export {}", <$ty as TS>::decl($cfg))),*]
    };
}

fn render() -> String {
    // serde_json writes 64-bit integers as JSON numbers; GMM's ids and sizes fit in a double.
    let cfg = &Config::new().with_large_int("number");
    let decls: Vec<String> = declarations![cfg;
        // Commands' own types.
        CmdError, GameInfo, CatalogQuery, CatalogPage, ModRow, LibraryView, ModKey, VariantPick,
        InstallArgs, InstallProgress, InstallResult, Toggled, UninstallItem, Uninstalled,
        GameStorageView, StorageView, LaunchInfo, Launched, FolderRow, Settings, StepState,
        StartupStep, StartupReport,
        // Setup (gmm-xxmi, gmm-detect).
        gmm_xxmi::GameStatus, gmm_xxmi::SetupState, gmm_xxmi::ImporterInfo,
        gmm_xxmi::ModsSummary, gmm_detect::GameInstall, gmm_detect::InstallSource,
        // Catalogue (gmm-catalog).
        gmm_catalog::Page<()>, gmm_catalog::Sort, gmm_catalog::Visibility, gmm_catalog::Member,
        gmm_catalog::CategoryRef, gmm_catalog::ImageSize, gmm_catalog::ImageVariant,
        gmm_catalog::Image, gmm_catalog::ModStats, gmm_catalog::ModSummary,
        gmm_catalog::ModFile, gmm_catalog::ModFiles, gmm_catalog::LicenseChecklist,
        gmm_catalog::Credit, gmm_catalog::CreditGroup, gmm_catalog::ModProfile,
        gmm_catalog::ChangeEntry, gmm_catalog::ModUpdate, gmm_catalog::Category,
        // Library, conflicts, storage, install (gmm-core).
        gmm_core::DiskState, gmm_core::UnmanagedMod, gmm_core::ModRef, gmm_core::Conflict,
        gmm_core::ConflictReport, gmm_core::Size, gmm_core::ModUsage, gmm_core::GameStorage,
        gmm_core::CacheStorage, gmm_core::ClearedCache, gmm_core::InstalledFolder,
        gmm_core::InstallReport, gmm_core::ToggleOutcome,
        // Two-step install, versions, queue, update check (gmm-core, Phase 5).
        gmm_core::InstallPlan, gmm_core::VersionPlan, gmm_core::VersionConflict,
        gmm_core::Job, gmm_core::JobState, gmm_core::ModUpdateInfo,
    ];
    let mut out = String::from(
        "// SPDX-License-Identifier: MIT OR Apache-2.0\n\
         // Generated from the Rust types by apps/desktop/src-tauri/src/bindings.rs. Don't edit;\n\
         // run `GMM_UPDATE_BINDINGS=1 cargo test -p gmm-desktop bindings` instead.\n",
    );
    for decl in decls {
        out.push('\n');
        out.push_str(&decl);
        out.push('\n');
    }
    out
}

#[test]
fn bindings_are_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts");
    let fresh = render();
    if std::env::var_os("GMM_UPDATE_BINDINGS").is_some() {
        std::fs::write(&path, &fresh).unwrap();
        return;
    }
    let current = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        current == fresh,
        "apps/desktop/src/bindings.ts is out of date; run \
         `GMM_UPDATE_BINDINGS=1 cargo test -p gmm-desktop bindings`"
    );
}
