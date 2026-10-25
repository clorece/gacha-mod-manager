// SPDX-License-Identifier: MIT OR Apache-2.0

//! Keeping a mod's saved in-game toggles when it's switched off and on again (security rule
//! R3, amended 2026-10-25).
//!
//! 3DMigoto saves a mod's toggle choices (an outfit picked with a key, a part hidden) in
//! `<IMP>/d3dx_user.ini`, under `[Constants]`, keyed by the path of the mod's `.ini`
//! (`$\mods\gmm\ellen [gb-1]\mod.ini\swapvar = 2`) or by the namespace it declares
//! (`$\<namespace>\swapvar`). Switching a mod off renames its folder, the keys stop matching,
//! and 3DMigoto drops them the next time it saves (docs/02, "Folder names must stay
//! stable"). So GMM copies a mod's lines into its database just before switching it off, and
//! writes the missing ones back just after switching it on. Every switch goes through
//! [`crate::set_enabled`] or [`crate::set_unmanaged_enabled_at`], so profiles, version
//! switches and Enable all keep the choices too.
//!
//! The write is the one place GMM touches a file outside `Mods/` in an importer folder, and
//! it is fenced in: only `d3dx_user.ini`, on a guard of its own; only lines GMM saved from
//! that same file are added, nothing is changed or removed; a copy of the old file goes to
//! `cache/backups/d3dx_user/` first (the newest ten are kept); and never while the game is
//! running, since the game rewrites the file from memory. A write-back that has to wait is
//! marked pending and done before GMM's next launch of the game, or at the next switch.
//! Reading or writing failures never stop a switch: the mod's choices then start fresh,
//! which is what happened before.

use std::sync::atomic::{AtomicU64, Ordering};

use gmm_adapters::GameManifest;
use gmm_db::Db;
use gmm_fsguard::{Access, Guard, RootId, ScopedPath};
use gmm_migoto::parse;

use crate::analyze::MAX_INI_BYTES;
use crate::error::CoreError;
use crate::keys::ini_files;
use crate::workspace::Workspace;

/// The only file GMM writes in an importer folder.
pub const USER_INI: &str = "d3dx_user.ini";
/// Where copies of the file go before each write, inside GMM's cache.
const BACKUP_DIR: &str = "backups/d3dx_user";
const KEEP_BACKUPS: usize = 10;
/// Far above what 3DMigoto writes (a few KiB); anything bigger is left alone.
const MAX_USER_INI_BYTES: u64 = 4 * 1024 * 1024;

static BACKUP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// What happened to a mod's saved choices when it was switched on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// Nothing was saved for it, or every line was already there.
    Nothing,
    /// The missing lines were written back.
    Written(usize),
    /// The game is running; the lines go back when it has closed.
    Waiting,
}

/// A mod's identity in `saved_toggles`: its path under `Mods/` in the switched-on form,
/// lowercased, `/` separators.
pub(crate) fn mod_key(rel_on: &str) -> String {
    rel_on.replace('\\', "/").to_lowercase()
}

/// Copies the mod's current lines from `d3dx_user.ini` into the database. Call it while the
/// mod is still on (`rel_on` names its switched-on path under `Mods/`). Finding nothing
/// keeps what was saved before (choices made in an earlier session), and clears a pending
/// write-back, since the mod is going off again.
pub(crate) async fn save_before_off(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    rel_on: &str,
    is_dir: bool,
) -> Result<usize, CoreError> {
    let key = mod_key(rel_on);
    let Some(ini) = UserIni::open(ws, mods)? else {
        return Ok(0);
    };
    let Some(text) = ini.read()? else {
        return Ok(0);
    };
    let prefixes = mod_prefixes(ws.guard(), mods, rel_on, is_dir)?;
    let lines = lines_with_prefixes(&text, &prefixes);
    if lines.is_empty() {
        db.set_toggles_pending(game.id(), &key, false).await?;
        return Ok(0);
    }
    db.save_toggles(game.id(), &key, &lines.join("\n")).await?;
    Ok(lines.len())
}

/// Writes the mod's saved lines back that `d3dx_user.ini` doesn't have. Call it once the
/// mod is on again.
pub(crate) async fn restore_after_on(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    rel_on: &str,
) -> Result<Restored, CoreError> {
    let key = mod_key(rel_on);
    let Some(saved) = db.saved_toggles(game.id(), &key).await? else {
        return Ok(Restored::Nothing);
    };
    if gmm_xxmi::game_running(game) {
        db.set_toggles_pending(game.id(), &key, true).await?;
        return Ok(Restored::Waiting);
    }
    // Write-backs that waited for the game to close go along in the same write.
    let pending = db.pending_toggles(game.id()).await?;
    let lines: Vec<String> = std::iter::once(&saved)
        .chain(pending.iter().filter(|p| p.mod_path != key))
        .flat_map(|p| p.lines.lines().map(str::to_owned))
        .collect();
    let written = write_back(ws, mods, &lines)?;
    db.set_toggles_pending(game.id(), &key, false).await?;
    for p in &pending {
        db.set_toggles_pending(game.id(), &p.mod_path, false)
            .await?;
    }
    Ok(written.map_or(Restored::Nothing, Restored::Written))
}

/// Does the write-backs that waited for the game to close. Returns how many mods got their
/// lines back; 0 while the game is still running.
pub async fn restore_pending(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
) -> Result<usize, CoreError> {
    let pending = db.pending_toggles(game.id()).await?;
    if pending.is_empty() || gmm_xxmi::game_running(game) {
        return Ok(0);
    }
    let lines: Vec<String> = pending
        .iter()
        .flat_map(|p| p.lines.lines().map(str::to_owned))
        .collect();
    write_back(ws, mods, &lines)?;
    for p in &pending {
        db.set_toggles_pending(game.id(), &p.mod_path, false)
            .await?;
    }
    Ok(pending.len())
}

/// Forgets what was saved for a mod that was uninstalled.
pub(crate) async fn forget(db: &Db, game: &GameManifest, rel_on: &str) -> Result<(), CoreError> {
    db.forget_toggles(game.id(), &mod_key(rel_on)).await?;
    Ok(())
}

/// Merges `lines` into the file and writes it (after a backup) when anything was missing.
/// Returns how many lines were added, or `None` when nothing needed writing.
fn write_back(ws: &Workspace, mods: RootId, lines: &[String]) -> Result<Option<usize>, CoreError> {
    let Some(ini) = UserIni::open(ws, mods)? else {
        return Ok(None);
    };
    let old = ini.read()?;
    let Some((merged, added)) = merge(old.as_deref(), lines) else {
        return Ok(None);
    };
    if let Some(old) = &old {
        backup(ws, &ini.importer, old)?;
    }
    ini.guard.write_atomic(&ini.file, merged.as_bytes())?;
    Ok(Some(added))
}

/// `d3dx_user.ini` of the importer that holds `mods`, on a guard of its own so nothing else
/// in the importer folder is ever writable.
struct UserIni {
    guard: Guard,
    file: ScopedPath,
    /// The importer folder's name (`ZZMI`), for backup file names.
    importer: String,
}

impl UserIni {
    fn open(ws: &Workspace, mods: RootId) -> Result<Option<Self>, CoreError> {
        let Some(importer_dir) = ws.guard().root_path(mods).and_then(|p| p.parent()) else {
            return Ok(None);
        };
        let importer = importer_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "importer".to_owned());
        let mut guard = Guard::new();
        let root = guard.register_root("importer", importer_dir, Access::ReadWrite)?;
        let file = guard.path(root, USER_INI)?;
        Ok(Some(Self {
            guard,
            file,
            importer,
        }))
    }

    /// The file's text, or `None` when it doesn't exist (3DMigoto creates it on its first
    /// save) or is implausibly large.
    fn read(&self) -> Result<Option<String>, CoreError> {
        if !self.guard.exists(&self.file)? {
            return Ok(None);
        }
        if self.guard.metadata(&self.file)?.len() > MAX_USER_INI_BYTES {
            return Ok(None);
        }
        let bytes = self.guard.read(&self.file)?;
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
    }
}

/// Keeps a copy of the file before GMM changes it, and only the newest few copies.
fn backup(ws: &Workspace, importer: &str, text: &str) -> Result<(), CoreError> {
    let guard = ws.guard();
    let dir = guard.path(ws.cache_root(), BACKUP_DIR)?;
    guard.create_dir_all(&dir)?;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let n = BACKUP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("{importer}-{secs:012}-{n:04}.ini");
    guard.write_atomic(&dir.join(&name)?, text.as_bytes())?;

    let prefix = format!("{importer}-");
    let mut copies: Vec<_> = guard
        .read_dir(&dir)?
        .into_iter()
        .filter(|e| !e.is_dir && !e.is_symlink && e.name.starts_with(&prefix))
        .collect();
    copies.sort_by(|a, b| b.name.cmp(&a.name));
    for old in copies.into_iter().skip(KEEP_BACKUPS) {
        let _ = guard.remove_file(&old.path);
    }
    Ok(())
}

/// The key prefixes of a mod: its path (every `.ini` inside it) and each namespace its
/// `.ini` files declare. Lowercase, like 3DMigoto writes them.
fn mod_prefixes(
    guard: &Guard,
    mods: RootId,
    rel_on: &str,
    is_dir: bool,
) -> Result<Vec<String>, CoreError> {
    let path = rel_on.replace('/', "\\").to_lowercase();
    let mut prefixes = vec![format!("$\\mods\\{path}\\")];
    let at = guard.path(mods, rel_on)?;
    let files = if is_dir {
        ini_files(guard, &at, &mut Vec::new())
    } else {
        vec![(at, String::new())]
    };
    for (file, _) in files {
        let small = guard
            .metadata(&file)
            .is_ok_and(|m| m.len() <= MAX_INI_BYTES);
        let Some(bytes) = small.then(|| guard.read(&file).ok()).flatten() else {
            continue;
        };
        if let Some(ns) = parse(&String::from_utf8_lossy(&bytes)).namespace {
            let ns = ns.replace('/', "\\").to_lowercase();
            let prefix = format!("$\\{}\\", ns.trim_matches('\\'));
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
    }
    Ok(prefixes)
}

/// The lowercase key of a `$\...\name = value` line.
fn key_of(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with('$') {
        return None;
    }
    let (key, _) = line.split_once('=')?;
    Some(key.trim().to_lowercase())
}

/// The lines (trimmed) whose key starts with one of `prefixes`.
fn lines_with_prefixes(text: &str, prefixes: &[String]) -> Vec<String> {
    text.lines()
        .filter(|line| key_of(line).is_some_and(|k| prefixes.iter().any(|p| k.starts_with(p))))
        .map(|line| line.trim().to_owned())
        .collect()
}

/// `text` with the lines whose keys it doesn't have yet added at the end of its
/// `[Constants]` section (created when missing), keeping the file's line endings. `None`
/// when every key is already there.
fn merge(text: Option<&str>, lines: &[String]) -> Option<(String, usize)> {
    let text = text.unwrap_or("");
    let present: std::collections::HashSet<String> = text.lines().filter_map(key_of).collect();
    let mut seen = std::collections::HashSet::new();
    let missing: Vec<&str> = lines
        .iter()
        .filter(|l| key_of(l).is_some_and(|k| !present.contains(&k) && seen.insert(k)))
        .map(|l| l.trim())
        .collect();
    if missing.is_empty() {
        return None;
    }
    let eol = if text.contains("\r\n") || text.is_empty() {
        "\r\n"
    } else {
        "\n"
    };
    let mut out: Vec<&str> = text.lines().collect();
    let section = out
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case("[constants]"));
    match section {
        Some(start) => {
            // The end of the section: the next `[...]` header, minus trailing blank lines.
            let mut end = out[start + 1..]
                .iter()
                .position(|l| l.trim_start().starts_with('['))
                .map_or(out.len(), |i| start + 1 + i);
            while end > start + 1 && out[end - 1].trim().is_empty() {
                end -= 1;
            }
            out.splice(end..end, missing.iter().copied());
        }
        None => {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push("");
            }
            out.push("[Constants]");
            out.extend(missing.iter().copied());
        }
    }
    let mut merged = out.join(eol);
    merged.push_str(eol);
    Some((merged, missing.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "; AUTOMATICALLY GENERATED FILE - DO NOT EDIT\r\n;\r\n[Constants]\r\n\
        $\\wwmiv1\\shown = 1\r\n\
        $\\mods\\gmm\\aemeath [gb-1]\\mod.ini\\swapvar = 2\r\n\
        $\\mods\\gmm\\aemeath [gb-1]\\parts\\extra.ini\\hat = 0\r\n\
        $\\mods\\gmm\\aemeath [gb-10]\\mod.ini\\swapvar = 5\r\n\
        $\\aemeathlib\\glow = 1\r\n";

    #[test]
    fn a_mods_lines_are_found_by_path_and_namespace_only() {
        let prefixes = vec![
            "$\\mods\\gmm\\aemeath [gb-1]\\".to_owned(),
            "$\\aemeathlib\\".to_owned(),
        ];
        assert_eq!(
            lines_with_prefixes(FILE, &prefixes),
            [
                "$\\mods\\gmm\\aemeath [gb-1]\\mod.ini\\swapvar = 2",
                "$\\mods\\gmm\\aemeath [gb-1]\\parts\\extra.ini\\hat = 0",
                "$\\aemeathlib\\glow = 1",
            ],
            "the [gb-10] mod and XXMI's own keys are left out"
        );
    }

    #[test]
    fn merge_adds_only_missing_keys_inside_constants() {
        let saved = vec![
            "$\\mods\\gmm\\aemeath [gb-1]\\mod.ini\\swapvar = 9".to_owned(),
            "$\\mods\\gmm\\new [gb-2]\\mod.ini\\swapvar = 3".to_owned(),
        ];
        let text = format!("{FILE}\r\n[Other]\r\nx = 1\r\n");
        let (merged, added) = merge(Some(&text), &saved).unwrap();
        assert_eq!(added, 1, "a key the file has keeps the file's value");
        assert!(merged.contains("swapvar = 2") && !merged.contains("swapvar = 9"));
        let new_at = merged.find("new [gb-2]").unwrap();
        assert!(
            new_at < merged.find("[Other]").unwrap(),
            "inside [Constants]"
        );
        assert!(!merged.replace("\r\n", "").contains('\n'), "CRLF kept");
        assert_eq!(merge(Some(&merged), &saved), None, "nothing left to add");
    }

    #[test]
    fn merge_creates_the_section_and_file_when_missing() {
        let saved = vec!["$\\mods\\a\\mod.ini\\v = 1".to_owned()];
        let (merged, _) = merge(None, &saved).unwrap();
        assert_eq!(merged, "[Constants]\r\n$\\mods\\a\\mod.ini\\v = 1\r\n");
        let (merged, _) = merge(Some("; header\n"), &saved).unwrap();
        assert_eq!(
            merged,
            "; header\n\n[Constants]\n$\\mods\\a\\mod.ini\\v = 1\n"
        );
    }

    #[test]
    fn keys_compare_without_case_or_spaces() {
        assert_eq!(
            key_of("  $\\Mods\\GMM\\X\\Mod.ini\\SwapVar   =  1 ").as_deref(),
            Some("$\\mods\\gmm\\x\\mod.ini\\swapvar")
        );
        assert_eq!(key_of("; $\\not = a key"), None);
        assert_eq!(mod_key("GMM\\Ellen [gb-1]"), "gmm/ellen [gb-1]");
    }
}
