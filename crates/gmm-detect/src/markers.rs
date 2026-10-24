// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use gmm_adapters::GameManifest;
use gmm_fsguard::{Access, Guard};

/// True if `folder` itself holds `game`: one of its exe names directly inside, and every
/// required child present.
pub fn is_game_folder(game: &GameManifest, folder: &Path) -> bool {
    check(game, folder).is_some()
}

/// `folder` if it's a game folder, else the first of the manifest's typical sub-folder names
/// that is (launchers often install into `<chosen folder>/<Game Name Game>`). Returns the
/// canonical path in plain form.
pub(crate) fn resolve_game_folder(game: &GameManifest, folder: &Path) -> Option<PathBuf> {
    check(game, folder).or_else(|| {
        game.detect
            .folder_names
            .iter()
            .find_map(|name| check(game, &folder.join(name)))
    })
}

fn check(game: &GameManifest, folder: &Path) -> Option<PathBuf> {
    if !folder.is_absolute() {
        return None;
    }
    let mut guard = Guard::new();
    let root = guard.register_root("game", folder, Access::ReadOnly).ok()?;
    let is_file = |rel: &str| {
        guard
            .path(root, rel)
            .ok()
            .and_then(|p| guard.metadata(&p).ok())
            .is_some_and(|m| m.is_file())
    };
    let exists = |rel: &str| {
        guard
            .path(root, rel)
            .ok()
            .and_then(|p| guard.exists(&p).ok())
            .unwrap_or(false)
    };
    let has_exe = game.detect.exe_names.iter().any(|exe| is_file(exe));
    let has_children = game.detect.required_children.iter().all(|c| exists(c));
    if !(has_exe && has_children) {
        return None;
    }
    guard.root_path(root).map(|p| plain(p.to_path_buf()))
}

/// Drops Windows' `\\?\` verbatim prefix that canonicalization adds, so paths compare and
/// display like the ones users (and XXMI's config) write.
pub(crate) fn plain(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}
