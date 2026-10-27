// SPDX-License-Identifier: MIT OR Apache-2.0

//! Installing a mod from an archive on the user's computer (Phase 6: a file dropped on the
//! window, or picked in a file dialog). It goes through the same pipeline as a download:
//! the archive is copied into GMM's cache (read-only access to its folder, which is
//! registered with the guard for this one read), then extracted with every safety check,
//! scanned and classified; the user confirms the versions; the chosen ones are installed
//! like any other mod, without GameBanana ids. The path comes from the OS (the drop or the
//! dialog, handled in Rust), never from the webview (security rule R5).

use std::io::{Read, Write};
use std::path::Path;

use gmm_adapters::GameManifest;
use gmm_db::Db;
use gmm_fsguard::{Access, Guard, RootId, ScopedPath};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::analyze::{Analysis, VersionConflict, analyze};
use crate::error::CoreError;
use crate::fetch::MAX_DOWNLOAD_BYTES;
use crate::install::{InstallEvent, InstallOptions, InstalledFolder, ModMeta, install_analyzed};
use crate::plan::{VersionPlan, install_to, version_plans};
use crate::workspace::{Workspace, unique_id};

/// Archive types a local install accepts (the ones GMM extracts).
pub const LOCAL_ARCHIVE_TYPES: [&str; 3] = ["zip", "7z", "rar"];

/// Longest mod name taken from a file name.
const MAX_LOCAL_NAME: usize = 80;

/// A local archive copied into the cache and analysed, waiting for the confirmation. Its
/// copy and extraction folder stay in the cache until [`LocalPrepared::discard`].
#[derive(Debug)]
pub struct LocalPrepared {
    /// From the file name, without the extension.
    pub name: String,
    pub file_name: String,
    pub archive: ScopedPath,
    pub sha256: String,
    pub bytes: u64,
    pub analysis: Analysis,
}

impl LocalPrepared {
    /// Removes the extraction folder and the cached copy (the user's file is untouched).
    pub fn discard(&self, ws: &Workspace) {
        self.analysis.cleanup(ws);
        let _ = ws.guard().remove_file(&self.archive);
    }
}

/// What the local install confirmation shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LocalPlan {
    pub game: String,
    pub name: String,
    pub file_name: String,
    pub archive_bytes: u64,
    /// The mod kind, or `None` when nothing in the archive is a mod for this game.
    pub kind: Option<String>,
    pub versions: Vec<VersionPlan>,
    pub version_conflicts: Vec<VersionConflict>,
    pub install_to: String,
}

fn mod_name_from(file_name: &str) -> String {
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let cleaned: String = stem
        .chars()
        .map(|c| if c == '_' { ' ' } else { c })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let name: String = words.join(" ").chars().take(MAX_LOCAL_NAME).collect();
    if name.is_empty() {
        "Local mod".to_owned()
    } else {
        name
    }
}

/// Copies `file` (an absolute path from a drop or a file dialog) into the cache and analyses
/// it for `game`.
pub fn prepare_local(
    ws: &Workspace,
    game: &GameManifest,
    file: &Path,
) -> Result<LocalPrepared, CoreError> {
    let refuse = |why: String| CoreError::CantInstallLocal(why);
    let file_name = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| refuse("that isn't a file name GMM can read".to_owned()))?
        .to_owned();
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !LOCAL_ARCHIVE_TYPES.contains(&extension.as_str()) {
        return Err(refuse(format!(
            "{file_name} isn't a .zip, .7z or .rar archive"
        )));
    }
    let folder = file
        .parent()
        .ok_or_else(|| refuse("the file has no folder".to_owned()))?;

    // Read-only access to the file's folder, for this copy only.
    let mut source_guard = Guard::new();
    let root: RootId = source_guard.register_root("dropped file", folder, Access::ReadOnly)?;
    let source = source_guard.path(root, &file_name)?;
    let meta = source_guard.metadata(&source)?;
    if !meta.is_file() {
        return Err(refuse(format!("{file_name} isn't a regular file")));
    }
    if meta.len() > MAX_DOWNLOAD_BYTES {
        return Err(refuse(format!("{file_name} is larger than GMM installs")));
    }

    let guard = ws.guard();
    let downloads = guard.path(ws.cache_root(), "downloads")?;
    guard.create_dir_all(&downloads)?;
    let part = downloads.join(&format!(".local-{}.part", unique_id()))?;
    let copied = (|| {
        let mut input = source_guard.open_read(&source)?;
        let mut output = guard.create_new(&part)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 256 * 1024];
        let mut total = 0u64;
        loop {
            let n = input
                .read(&mut buffer)
                .map_err(|e| CoreError::Io(e.to_string()))?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > MAX_DOWNLOAD_BYTES {
                return Err(refuse(format!("{file_name} is larger than GMM installs")));
            }
            hasher.update(&buffer[..n]);
            output
                .write_all(&buffer[..n])
                .map_err(|e| CoreError::Io(e.to_string()))?;
        }
        output
            .sync_all()
            .map_err(|e| CoreError::Io(e.to_string()))?;
        Ok::<_, CoreError>((hex::encode(hasher.finalize()), total))
    })();
    let (sha256, bytes) = match copied {
        Ok(done) => done,
        Err(e) => {
            let _ = guard.remove_file(&part);
            return Err(e);
        }
    };
    // Named by content, so dropping the same file twice reuses one copy.
    let archive = downloads.join(&format!("local-{}.{extension}", &sha256[..16]))?;
    if guard.exists(&archive)? {
        let _ = guard.remove_file(&part);
    } else {
        guard.rename(&part, &archive)?;
    }
    let analysis = match analyze(ws, game, &archive) {
        Ok(a) => a,
        Err(e) => {
            let _ = guard.remove_file(&archive);
            return Err(e);
        }
    };
    Ok(LocalPrepared {
        name: mod_name_from(&file_name),
        file_name,
        archive,
        sha256,
        bytes,
        analysis,
    })
}

/// The confirmation for a prepared local archive.
pub async fn local_plan(
    db: &Db,
    game: &GameManifest,
    prepared: &LocalPrepared,
) -> Result<LocalPlan, CoreError> {
    let analysis = &prepared.analysis;
    Ok(LocalPlan {
        game: game.id().to_owned(),
        name: prepared.name.clone(),
        file_name: prepared.file_name.clone(),
        archive_bytes: prepared.bytes,
        kind: analysis.kind.clone(),
        versions: version_plans(db, game, analysis, &prepared.name, None, None).await?,
        version_conflicts: analysis.version_conflicts.clone(),
        install_to: install_to(game),
    })
}

/// Installs the chosen versions of a prepared local archive. Doesn't discard `prepared`.
pub async fn install_local(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    prepared: &LocalPrepared,
    options: &InstallOptions,
    on_event: impl FnMut(InstallEvent),
) -> Result<Vec<InstalledFolder>, CoreError> {
    let meta = ModMeta {
        name: prepared.name.clone(),
        file_name: Some(prepared.file_name.clone()),
        archive_sha256: prepared.sha256.clone(),
        ..ModMeta::default()
    };
    install_analyzed(
        db,
        ws,
        mods,
        game,
        &meta,
        &prepared.analysis,
        options,
        on_event,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_the_file_name() {
        assert_eq!(mod_name_from("ellen_maid_v2.zip"), "ellen maid v2");
        assert_eq!(mod_name_from("  .rar"), "Local mod");
        assert_eq!(mod_name_from("Phoenix  Bride.7z"), "Phoenix Bride");
    }
}
