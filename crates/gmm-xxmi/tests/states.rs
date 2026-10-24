// SPDX-License-Identifier: MIT OR Apache-2.0

//! One fixture per setup state, built in a temp folder: a fake XXMI root (with secret canaries in
//! its config) and fake game installs.

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use gmm_adapters::Registry;
use gmm_detect::{Environment, GameInstall, InstallSource};
use gmm_xxmi::{SetupState, evaluate, find_xxmi};

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

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn json_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

struct World {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl World {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        Self { _tmp: tmp, root }
    }

    fn zzz_folder(&self) -> PathBuf {
        let f = self.root.join("HoYoverse").join("ZenlessZoneZero Game");
        write(&f.join("ZenlessZoneZero.exe"), b"MZ");
        fs::create_dir_all(f.join("ZenlessZoneZero_Data")).unwrap();
        f
    }

    fn xxmi_root(&self) -> PathBuf {
        self.root.join("XXMI Launcher")
    }

    /// XXMI with a launcher exe and a config. `zzmi` = (game_folder, game_launch, launch_count).
    fn xxmi(&self, zzmi: Option<(Option<&Path>, &str, i64)>) {
        let root = self.xxmi_root();
        write(&root.join("Resources/Bin/XXMI Launcher.exe"), b"MZ\x90\x00");
        let importer = match zzmi {
            Some((folder, launch, count)) => format!(
                r#""ZZMI": {{ "Importer": {{ "game_folder": "{}", "game_launch": "{launch}",
                    "launch_count": {count}, "run_pre_launch_signature": "CANARY_SIG" }} }}"#,
                folder.map(json_path).unwrap_or_default()
            ),
            None => String::new(),
        };
        let config = format!(
            r#"{{ "Launcher": {{ "config_version": "2.4.9", "github_token": "ghp_CANARY",
                   "proxy": {{ "password": "CANARY_PW" }} }},
                 "Importers": {{ {importer} }},
                 "Security": {{ "user_signature": "CANARY_USER_SIG" }} }}"#
        );
        write(&root.join("XXMI Launcher Config.json"), config.as_bytes());
    }

    /// A ZZMI importer folder; `target` is what d3dx.ini targets.
    fn zzmi(&self, target: &str, with_mods: bool) {
        let dir = self.xxmi_root().join("ZZMI");
        write(&dir.join("d3d11.dll"), b"not a real dll");
        write(
            &dir.join("d3dx.ini"),
            format!(
                "[Loader]\ntarget = {target}\n[Include]\ninclude = Core\\ZZMI\\main.ini\n{}",
                if with_mods {
                    "include_recursive = Mods\n"
                } else {
                    ""
                }
            )
            .as_bytes(),
        );
        write(&dir.join("Core/ZZMI/main.ini"), b"global $version = 1.50\n");
        write(
            &dir.join("Mods/Phoenix/resources/Phoenix.ini"),
            &[b'x'; 1000],
        );
        write(&dir.join("Mods/DISABLED Old Outfit/a.ini"), &[b'y'; 500]);
    }

    fn status(&self, installs: &[GameInstall]) -> gmm_xxmi::GameStatus {
        let zzz = Registry::builtin().unwrap().get("zzz").unwrap();
        let xxmi = find_xxmi(&NoEnv, Some(&self.xxmi_root()));
        evaluate(zzz, installs, xxmi.as_ref())
    }
}

fn install(folder: &Path, source: InstallSource) -> GameInstall {
    GameInstall {
        game_id: "zzz".to_owned(),
        source,
        folder: fs::canonicalize(folder)
            .map(|p| PathBuf::from(p.display().to_string().trim_start_matches(r"\\?\")))
            .unwrap(),
        steam_appid: None,
    }
}

fn assert_no_secrets(status: &gmm_xxmi::GameStatus) {
    let all = format!("{status:?} {}", serde_json::to_string(status).unwrap());
    assert!(!all.contains("CANARY"), "secret leaked: {all}");
}

#[test]
fn xxmi_not_found() {
    let w = World::new();
    let s = w.status(&[]);
    assert_eq!(s.state, SetupState::XxmiNotFound);
    assert!(s.steps[0].contains("github.com/SpectrumQT/XXMI-Launcher/releases"));
    assert!(
        s.notes
            .iter()
            .any(|n| n.contains("didn't find Zenless Zone Zero"))
    );
}

#[test]
fn importer_missing() {
    let w = World::new();
    w.xxmi(None);
    let s = w.status(&[]);
    assert_eq!(s.state, SetupState::ImporterMissing);
    assert!(s.steps.iter().any(|st| st.contains("Select Games To Mod:")));
    assert_eq!(s.xxmi_version.as_deref(), Some("2.4.9"));
    assert_no_secrets(&s);
}

#[test]
fn importer_broken() {
    let w = World::new();
    w.xxmi(None);
    w.zzmi("Client-Win64-Shipping.exe", true);
    assert_eq!(w.status(&[]).state, SetupState::ImporterBroken);
    let w = World::new();
    w.xxmi(None);
    w.zzmi("ZenlessZoneZero.exe", false);
    let s = w.status(&[]);
    assert_eq!(s.state, SetupState::ImporterBroken);
    assert!(
        s.summary.contains("doesn't load the Mods folder"),
        "{}",
        s.summary
    );
}

#[test]
fn game_path_not_set_suggests_detected_folder() {
    let w = World::new();
    let game = w.zzz_folder();
    w.xxmi(Some((None, "DIRECT", 0)));
    w.zzmi("ZenlessZoneZero.exe", true);
    let detected = install(&game, InstallSource::HoYoPlay);
    let s = w.status(std::slice::from_ref(&detected));
    assert_eq!(s.state, SetupState::GamePathNotSet);
    assert!(
        s.steps
            .iter()
            .any(|st| st.contains(&detected.folder.display().to_string())),
        "{:?}",
        s.steps
    );
    assert_no_secrets(&s);
}

#[test]
fn path_mismatch() {
    let w = World::new();
    let wrong = w.root.join("Empty");
    fs::create_dir_all(&wrong).unwrap();
    w.xxmi(Some((Some(&wrong), "DIRECT", 1)));
    w.zzmi("ZenlessZoneZero.exe", true);
    assert_eq!(w.status(&[]).state, SetupState::PathMismatch);
}

#[test]
fn steam_install_with_direct_launch_is_a_mismatch() {
    let w = World::new();
    let game = w.zzz_folder();
    w.xxmi(Some((Some(&game), "DIRECT", 2)));
    w.zzmi("ZenlessZoneZero.exe", true);
    let s = w.status(&[install(&game, InstallSource::Steam)]);
    assert_eq!(s.state, SetupState::LaunchModeMismatch);
    assert!(
        s.steps[0].contains("\"Game Launch\" to \"Steam\""),
        "{:?}",
        s.steps
    );
    assert_eq!(s.xxmi_game_folder_source, Some(InstallSource::Steam));
}

#[test]
fn hoyoplay_install_with_steam_launch_is_a_mismatch() {
    let w = World::new();
    let game = w.zzz_folder();
    w.xxmi(Some((Some(&game), "STEAM", 2)));
    w.zzmi("ZenlessZoneZero.exe", true);
    let s = w.status(&[install(&game, InstallSource::HoYoPlay)]);
    assert_eq!(s.state, SetupState::LaunchModeMismatch);
    assert!(s.steps[0].contains("\"Direct\""), "{:?}", s.steps);
}

#[test]
fn ready_with_details() {
    let w = World::new();
    let game = w.zzz_folder();
    w.xxmi(Some((Some(&game), "DIRECT", 3)));
    w.zzmi("ZenlessZoneZero.exe", true);
    let s = w.status(&[install(&game, InstallSource::HoYoPlay)]);
    assert_eq!(s.state, SetupState::Ready, "{} {:?}", s.summary, s.steps);
    assert!(s.steps.is_empty());
    let info = s.importer_info.as_ref().unwrap();
    assert_eq!(info.importer_version.as_deref(), Some("1.50"));
    assert_eq!((info.mods.enabled, info.mods.disabled), (1, 1));
    assert_eq!(info.mods.total_bytes, 1500);
    assert_eq!(info.dll_version, None, "fake dll has no version resource");
    let command = s.launch_command.as_ref().unwrap();
    assert!(command[0].ends_with("XXMI Launcher.exe"));
    assert_eq!(&command[1..], ["--nogui", "--xxmi", "ZZMI"]);
    assert_no_secrets(&s);
}

#[test]
fn ready_notes_first_launch_and_other_installs() {
    let w = World::new();
    let game = w.zzz_folder();
    let other = w.root.join("Steam").join("ZenlessZoneZero Game");
    write(&other.join("ZenlessZoneZero.exe"), b"MZ");
    fs::create_dir_all(other.join("ZenlessZoneZero_Data")).unwrap();
    w.xxmi(Some((Some(&game), "DIRECT", 0)));
    w.zzmi("ZenlessZoneZero.exe", true);
    let s = w.status(&[
        install(&game, InstallSource::HoYoPlay),
        install(&other, InstallSource::Steam),
    ]);
    assert_eq!(s.state, SetupState::Ready);
    assert!(
        s.notes.iter().any(|n| n.contains("hasn't started")),
        "{:?}",
        s.notes
    );
    assert!(
        s.notes
            .iter()
            .any(|n| n.contains("Another Zenless Zone Zero install"))
    );
}

#[test]
fn undetected_but_valid_folder_skips_launch_check() {
    let w = World::new();
    let game = w.zzz_folder();
    w.xxmi(Some((Some(&game), "STEAM", 1)));
    w.zzmi("ZenlessZoneZero.exe", true);
    let s = w.status(&[]);
    assert_eq!(s.state, SetupState::Ready);
    assert!(
        s.notes
            .iter()
            .any(|n| n.contains("launch setting wasn't checked"))
    );
}

/// WWMI's core ini isn't `main.ini` (observed on the dev machine); the check follows
/// d3dx.ini's include line instead of assuming a name.
#[test]
fn wwmi_layout_is_recognised() {
    let w = World::new();
    w.xxmi(None);
    let dir = w.xxmi_root().join("WWMI");
    write(&dir.join("d3d11.dll"), b"x");
    write(
        &dir.join("d3dx.ini"),
        b"[Loader]\ntarget = Client-Win64-Shipping.exe\n\
          include = Core\\WWMI\\WuWa-Model-Importer.ini\ninclude_recursive = Mods\n",
    );
    write(
        &dir.join("Core/WWMI/WuWa-Model-Importer.ini"),
        b"global $wwmi_version = 1.10\nglobal $required_wwmi_version = 0\n",
    );
    let wuwa = Registry::builtin().unwrap().get("wuwa").unwrap();
    let xxmi = find_xxmi(&NoEnv, Some(&w.xxmi_root()));
    let s = evaluate(wuwa, &[], xxmi.as_ref());
    assert_eq!(
        s.state,
        SetupState::GamePathNotSet,
        "{} {:?}",
        s.summary,
        s.notes
    );
    let info = s.importer_info.unwrap();
    assert!(info.missing_files.is_empty(), "{:?}", info.missing_files);
    assert_eq!(info.importer_version.as_deref(), Some("1.10"));
}

#[test]
fn importer_without_core_include_is_incomplete() {
    let w = World::new();
    w.xxmi(None);
    let dir = w.xxmi_root().join("ZZMI");
    write(&dir.join("d3d11.dll"), b"x");
    write(
        &dir.join("d3dx.ini"),
        b"target = ZenlessZoneZero.exe\ninclude_recursive = Mods\n",
    );
    let s = w.status(&[]);
    assert_eq!(s.state, SetupState::ImporterMissing);
    assert!(
        s.notes.iter().any(|n| n.contains("includes none")),
        "{:?}",
        s.notes
    );
}

#[test]
fn folder_without_exe_is_not_xxmi() {
    let w = World::new();
    write(&w.xxmi_root().join("XXMI Launcher Config.json"), b"{}");
    assert_eq!(w.status(&[]).state, SetupState::XxmiNotFound);
}
