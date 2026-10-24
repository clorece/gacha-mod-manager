// SPDX-License-Identifier: MIT OR Apache-2.0

//! Steam: `SteamPath` → `steamapps/libraryfolders.vdf` → each library's
//! `steamapps/appmanifest_<appid>.acf` → `steamapps/common/<installdir>/<subpath>`.
//! Nothing else under Steam is read (its login files are also on fsguard's denylist).

use std::path::PathBuf;

use gmm_adapters::Registry;
use gmm_fsguard::{Access, Guard, RelPath, RootId};

use crate::env::Environment;
use crate::{DetectReport, InstallSource, add_candidate, same_path, vdf};

pub(crate) fn detect(registry: &Registry, env: &dyn Environment, report: &mut DetectReport) {
    let Some(steam_root) = env.steam_root() else {
        return;
    };
    let mut guard = Guard::new();
    let Ok(steam) = guard.register_root("steam", &steam_root, Access::ReadOnly) else {
        return; // Steam isn't installed here.
    };

    let mut libraries = vec![steam_root.clone()];
    match read_vdf(&guard, steam, "steamapps/libraryfolders.vdf") {
        Ok(Some(doc)) => {
            if let Some(folders) = doc.get("libraryfolders") {
                for (_, library) in folders.entries() {
                    if let Some(path) = library.get("path").and_then(vdf::Value::text) {
                        let path = PathBuf::from(path);
                        if path.is_absolute() && !libraries.iter().any(|l| same_path(l, &path)) {
                            libraries.push(path);
                        }
                    }
                }
            }
        }
        Ok(None) => {}
        Err(e) => report
            .notes
            .push(format!("Steam library list unreadable: {e}")),
    }

    for library in libraries {
        let Ok(lib) = guard.register_root("steam library", &library, Access::ReadOnly) else {
            report.notes.push(format!(
                "Steam library {} is not available (unplugged drive?)",
                library.display()
            ));
            continue;
        };
        for game in registry.games() {
            let Some(steam_info) = &game.detect.steam else {
                continue;
            };
            let manifest = format!("steamapps/appmanifest_{}.acf", steam_info.appid);
            let doc = match read_vdf(&guard, lib, &manifest) {
                Ok(Some(doc)) => doc,
                Ok(None) => continue,
                Err(e) => {
                    report
                        .notes
                        .push(format!("{} in {}: {e}", manifest, library.display()));
                    continue;
                }
            };
            let Some(install_dir) = doc
                .get("AppState")
                .and_then(|a| a.get("installdir"))
                .and_then(vdf::Value::text)
            else {
                continue;
            };
            // installdir comes from a file on disk: it must be one plain folder name.
            let Ok(install_dir) = RelPath::parse(install_dir) else {
                continue;
            };
            let Ok(subpath) = RelPath::parse(&steam_info.subpath) else {
                continue;
            };
            if install_dir.depth() != 1 {
                continue;
            }
            let folder = library
                .join("steamapps")
                .join("common")
                .join(install_dir.to_path_buf())
                .join(subpath.to_path_buf());
            add_candidate(
                report,
                game,
                InstallSource::Steam,
                &folder,
                Some(steam_info.appid),
            );
        }
    }
}

/// `Ok(None)` when the file doesn't exist.
fn read_vdf(guard: &Guard, root: RootId, rel: &str) -> Result<Option<vdf::Value>, String> {
    let path = guard.path(root, rel).map_err(|e| e.to_string())?;
    if !guard.exists(&path).map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let text = guard.read_to_string(&path).map_err(|e| e.to_string())?;
    vdf::parse(&text).map(Some)
}
