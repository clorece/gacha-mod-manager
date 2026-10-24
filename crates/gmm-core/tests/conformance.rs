// SPDX-License-Identifier: MIT OR Apache-2.0

//! Adapter conformance suite (docs/03-game-adapters.md, "Adding a game"): every built-in game
//! goes through detect → setup state → install → toggle → conflicts → storage → uninstall
//! on a fake machine built from its manifest alone. Afterwards nothing outside the game's
//! `Mods/` folder (and GMM's own folders) may have changed, and none of the secrets in
//! XXMI's config may have reached GMM's database.

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use gmm_adapters::{GameManifest, Registry};
use gmm_core::{
    CoreError, DiskState, InstallOptions, ModMeta, Trash, Workspace, analyze, conflicts,
    game_storage, install_analyzed, install_gate, library, set_enabled, uninstall,
};
use gmm_db::{DB_FILE_NAME, Db};
use gmm_detect::{Environment, InstallSource, detect};
use gmm_fsguard::{Access, Guard, ScopedPath};
use gmm_xxmi::{SetupState, evaluate, open_xxmi};
use zip::write::{SimpleFileOptions, ZipWriter};

const CANARIES: &[&str] = &["ghp_CANARY", "CANARY_PW", "CANARY_SIG", "CANARY_USER_SIG"];

/// A machine with nothing on it: every game comes from a manual folder.
struct NoEnv;

impl Environment for NoEnv {
    fn steam_root(&self) -> Option<PathBuf> {
        None
    }
    fn roaming_app_data(&self) -> Option<PathBuf> {
        None
    }
    fn program_data(&self) -> Option<PathBuf> {
        None
    }
    fn xxmi_install_locations(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

#[derive(Default)]
struct FakeTrash(Mutex<Vec<String>>);

impl Trash for FakeTrash {
    fn trash(&self, guard: &Guard, path: &ScopedPath) -> Result<(), CoreError> {
        self.0.lock().unwrap().push(path.rel().to_string());
        guard.remove_dir_all(path)?;
        Ok(())
    }
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn json_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "\\\\")
}

/// The game folder the manifest describes: an exe and every required child (a name with an
/// extension is a file, anything else a folder).
fn fake_game(game: &GameManifest, dir: &Path) {
    write(&dir.join(&game.detect.exe_names[0]), b"MZ game");
    for child in &game.detect.required_children {
        let path = dir.join(child);
        if Path::new(child).extension().is_some() {
            write(&path, b"MZ");
        } else {
            fs::create_dir_all(&path).unwrap();
        }
    }
}

/// XXMI with this game's importer installed and pointed at `game_dir`, plus a hand-installed
/// mod and secret canaries in the config.
fn fake_xxmi(game: &GameManifest, root: &Path, game_dir: &Path) {
    let imp = game.xxmi.importer.as_str();
    write(&root.join("Resources/Bin/XXMI Launcher.exe"), b"MZ\x90\x00");
    let config = format!(
        r#"{{ "Launcher": {{ "config_version": "2.4.9", "github_token": "ghp_CANARY",
               "proxy": {{ "password": "CANARY_PW" }} }},
             "Importers": {{ "{imp}": {{ "Importer": {{ "game_folder": "{}",
               "game_launch": "DIRECT", "launch_count": 1,
               "run_pre_launch_signature": "CANARY_SIG" }} }} }},
             "Security": {{ "user_signature": "CANARY_USER_SIG" }} }}"#,
        json_path(game_dir)
    );
    write(&root.join("XXMI Launcher Config.json"), config.as_bytes());
    let dir = root.join(imp);
    write(&dir.join("d3d11.dll"), b"not a real dll");
    write(
        &dir.join("d3dx.ini"),
        format!(
            "[Loader]\ntarget = {}\n[Include]\ninclude = Core\\{imp}\\core.ini\n\
             include_recursive = Mods\nexclude_recursive = DISABLED*\n",
            game.xxmi.d3dx_target
        )
        .as_bytes(),
    );
    write(
        &dir.join("Core").join(imp).join("core.ini"),
        b"global $version = 1.0\n",
    );
    write(
        &dir.join("Mods/Manual/manual.ini"),
        b"[TextureOverrideManual]\nhash = 11111111\nib = R\n",
    );
}

fn mod_archive() -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        (
            "Conformance Mod/mod.ini",
            &b"[TextureOverrideBody]\nhash = 22222222\nib = ResourceIB\nps-t0 = ResourceTex\n"[..],
        ),
        ("Conformance Mod/body.ib", b"ib data"),
        ("Conformance Mod/tex.dds", b"DDS data"),
        ("readme.txt", b"thanks for downloading"),
    ] {
        w.start_file(name, SimpleFileOptions::default()).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// Every file and folder under `dir` except those below `skip`, with size and mtime.
fn snapshot(dir: &Path, skip: &Path) -> BTreeMap<PathBuf, (u64, Option<SystemTime>)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.starts_with(skip) {
                continue;
            }
            let meta = fs::symlink_metadata(&path).unwrap();
            out.insert(
                path.strip_prefix(dir).unwrap().to_path_buf(),
                (meta.len(), meta.modified().ok()),
            );
            if meta.is_dir() {
                stack.push(path);
            }
        }
    }
    out
}

async fn run_game(game: &GameManifest) {
    let id = game.id();
    let tmp = tempfile::tempdir().unwrap();
    let machine = tmp.path().join("machine");
    let game_dir = machine.join("Games").join(id);
    let xxmi_root = machine.join("AppData").join("XXMI Launcher");
    fake_game(game, &game_dir);
    fake_xxmi(game, &xxmi_root, &game_dir);
    let mods_dir = xxmi_root.join(game.xxmi.importer.as_str()).join("Mods");
    let before = snapshot(&machine, &mods_dir);

    // Detect and check the setup.
    let registry = Registry::builtin().unwrap();
    let report = detect(registry, &NoEnv, &[(id.to_owned(), game_dir.clone())]);
    let install = report
        .for_game(id)
        .next()
        .unwrap_or_else(|| panic!("{id}: manual folder not detected: {:?}", report.notes));
    assert_eq!(install.source, InstallSource::Manual);
    let xxmi = open_xxmi(&xxmi_root).unwrap();
    let status = evaluate(game, &report.installs, Some(&xxmi));
    assert_eq!(
        status.state,
        SetupState::Ready,
        "{id}: {} {:?}",
        status.summary,
        status.steps
    );
    assert!(install_gate(&status).unwrap().is_empty());

    // GMM's own folders and a database on disk (so the canary check reads a real file).
    let data_dir = tmp.path().join("gmm-data");
    let mut data_guard = Guard::new();
    let data = data_guard
        .ensure_root("data", &data_dir, Access::ReadWrite)
        .unwrap();
    let db = Db::open(&data_guard, &data_guard.path(data, DB_FILE_NAME).unwrap())
        .await
        .unwrap();
    let mut ws = Workspace::open(&tmp.path().join("gmm-cache")).unwrap();
    let mods = ws
        .add_mods_root(&xxmi.mods_dir(game.xxmi.importer))
        .unwrap();

    // Install.
    fs::write(
        tmp.path().join("gmm-cache/downloads/conformance.zip"),
        mod_archive(),
    )
    .unwrap();
    let archive = ws
        .guard()
        .path(ws.cache_root(), "downloads/conformance.zip")
        .unwrap();
    let analysis = analyze(&ws, game, &archive).unwrap();
    assert_eq!(analysis.kind.as_deref(), Some("migoto"), "{id}");
    let meta = ModMeta {
        name: "Conformance Mod".to_owned(),
        gb_mod_id: Some(1),
        archive_sha256: "00".repeat(32),
        ..ModMeta::default()
    };
    let installed = install_analyzed(
        &db,
        &ws,
        mods,
        game,
        &meta,
        &analysis,
        &InstallOptions::default(),
        |_| {},
    )
    .await
    .unwrap();
    analysis.cleanup(&ws);
    let mod_id = installed[0].id;
    let folder = mods_dir.join("GMM").join("Conformance Mod [gb-1]");
    assert!(folder.join("mod.ini").exists() && folder.join("gmm.json").exists());
    assert!(!folder.join("readme.txt").exists());

    // XXMI's view: the importer counts the GMM mod and the hand-installed one.
    let status = evaluate(game, &report.installs, Some(&xxmi));
    let counts = status.importer_info.as_ref().unwrap().mods.clone();
    assert_eq!((counts.enabled, counts.disabled), (2, 0), "{id}");

    // Library, toggle, conflicts, storage.
    let lib = library(&db, &ws, mods, game).await.unwrap();
    assert_eq!(lib.managed[0].disk, DiskState::Enabled);
    assert_eq!(lib.unmanaged.len(), 1);
    set_enabled(&db, &ws, mods, game, mod_id, false)
        .await
        .unwrap();
    assert!(
        mods_dir
            .join("GMM/DISABLED Conformance Mod [gb-1]/mod.ini")
            .exists()
    );
    let status = evaluate(game, &report.installs, Some(&xxmi));
    let counts = status.importer_info.as_ref().unwrap().mods.clone();
    assert_eq!((counts.enabled, counts.disabled), (1, 1), "{id}");
    set_enabled(&db, &ws, mods, game, mod_id, true)
        .await
        .unwrap();
    assert!(
        conflicts(&db, &ws, mods, game)
            .await
            .unwrap()
            .conflicts
            .is_empty()
    );
    let storage = game_storage(&db, &ws, mods, game).await.unwrap();
    assert_eq!(storage.managed.len(), 1);
    assert_eq!(storage.unmanaged.len(), 1);

    // Uninstall.
    let trash = FakeTrash::default();
    let out = uninstall(&db, &ws, mods, game, mod_id, &trash)
        .await
        .unwrap();
    assert!(out.trashed);
    assert!(!folder.exists());
    assert!(db.installed_mods(Some(id)).await.unwrap().is_empty());
    assert!(
        mods_dir.join("Manual/manual.ini").exists(),
        "{id}: unmanaged mod untouched"
    );
    db.close().await;

    // Nothing outside Mods/ changed, and no secret reached GMM's own files.
    assert_eq!(
        snapshot(&machine, &mods_dir),
        before,
        "{id}: write outside Mods/"
    );
    for dir in [data_dir, tmp.path().join("gmm-cache"), mods_dir] {
        for (path, _) in snapshot(&dir, Path::new("/nonexistent")) {
            let full = dir.join(&path);
            if full.is_file() {
                let bytes = fs::read(&full).unwrap();
                let text = String::from_utf8_lossy(&bytes);
                for canary in CANARIES {
                    assert!(!text.contains(canary), "{id}: {canary} found in {full:?}");
                }
            }
        }
    }
}

#[tokio::test]
async fn every_builtin_game_conforms() {
    let games = Registry::builtin().unwrap().games();
    assert!(games.len() >= 2);
    for game in games {
        run_game(game).await;
    }
}
