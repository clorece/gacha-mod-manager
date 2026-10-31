// SPDX-License-Identifier: MIT OR Apache-2.0

//! Detection against a fake machine built in a temp folder.

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use gmm_adapters::Registry;
use gmm_detect::{Environment, InstallSource, detect, is_game_folder, same_path};

struct FakeEnv {
    steam: Option<PathBuf>,
    app_data: Option<PathBuf>,
    program_data: Option<PathBuf>,
    gryphlink: Vec<PathBuf>,
}

impl Environment for FakeEnv {
    fn steam_root(&self) -> Option<PathBuf> {
        self.steam.clone()
    }
    fn roaming_app_data(&self) -> Option<PathBuf> {
        self.app_data.clone()
    }
    fn program_data(&self) -> Option<PathBuf> {
        self.program_data.clone()
    }
    fn xxmi_install_locations(&self) -> Vec<PathBuf> {
        Vec::new()
    }
    fn gryphlink_roots(&self) -> Vec<PathBuf> {
        self.gryphlink.clone()
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn make_zzz(folder: &Path) {
    write(&folder.join("ZenlessZoneZero.exe"), "MZ");
    fs::create_dir_all(folder.join("ZenlessZoneZero_Data")).unwrap();
}

fn make_endfield(folder: &Path) {
    write(&folder.join("Endfield.exe"), "MZ");
    fs::create_dir_all(folder.join("Endfield_Data")).unwrap();
}

fn make_wuwa(folder: &Path) {
    write(&folder.join("Wuthering Waves.exe"), "MZ");
    write(
        &folder.join("Client/Binaries/Win64/Client-Win64-Shipping.exe"),
        "MZ",
    );
    fs::create_dir_all(folder.join("Engine")).unwrap();
}

fn vdf_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

#[test]
fn finds_installs_from_every_source() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // Steam: main library (WuWa) plus a second library (ZZZ, nested sub-folder).
    let steam = root.join("Steam");
    let lib2 = root.join("D_drive").join("SteamLibrary");
    write(
        &steam.join("steamapps/libraryfolders.vdf"),
        &format!(
            "\"libraryfolders\"\n{{\n \"0\" {{ \"path\" \"{}\" }}\n \"1\" {{ \"path\" \"{}\" }}\n}}\n",
            vdf_path(&steam),
            vdf_path(&lib2)
        ),
    );
    write(
        &steam.join("steamapps/appmanifest_3513350.acf"),
        "\"AppState\" { \"appid\" \"3513350\" \"installdir\" \"Wuthering Waves\" }",
    );
    make_wuwa(&steam.join("steamapps/common/Wuthering Waves"));
    write(
        &lib2.join("steamapps/appmanifest_4162040.acf"),
        "\"AppState\" { \"appid\" \"4162040\" \"installdir\" \"Zenless Zone Zero\" }",
    );
    make_zzz(&lib2.join("steamapps/common/Zenless Zone Zero/games/ZenlessZoneZero Game"));
    // Canary: Steam's login file must never matter (and fsguard refuses to open it).
    write(&steam.join("config/loginusers.vdf"), "CANARY");

    // HoYoPlay: one real install, one default path for a game that isn't installed.
    let app_data = root.join("AppData");
    let hoyo_zzz = root.join("HoYoverse").join("ZenlessZoneZero Game");
    make_zzz(&hoyo_zzz);
    let mut data = vec![0x83, 0x70, 0x00, 0x00];
    data.extend_from_slice(
        format!(
            "{{\"gameBiz\":\"nap_global\",\"installPath\":\"{}\"}}\0{{\"installPath\":\"{}\"}}",
            vdf_path(&hoyo_zzz),
            vdf_path(&root.join("NotInstalled").join("Genshin Impact game"))
        )
        .as_bytes(),
    );
    let gamedata = app_data.join("Cognosphere/HYP/1_0/data/gamedata.dat");
    fs::create_dir_all(gamedata.parent().unwrap()).unwrap();
    fs::write(&gamedata, data).unwrap();

    // GRYPHLINK: the registry names the launcher's versioned folder, the game is in
    // <launcher>/games. A default folder that doesn't exist stays quiet.
    let gryphlink = root.join("GRYPHLINK");
    make_endfield(&gryphlink.join("games/EndField Game"));
    fs::create_dir_all(gryphlink.join("1.2.0")).unwrap();

    // Epic: WuWa installed into a folder that contains "Wuthering Waves Game".
    let program_data = root.join("ProgramData");
    let epic_wuwa = root.join("Epic").join("WuWa");
    make_wuwa(&epic_wuwa.join("Wuthering Waves Game"));
    write(
        &program_data.join("Epic/EpicGamesLauncher/Data/Manifests/ABC123.item"),
        &format!(
            "{{\"DisplayName\":\"Wuthering Waves\",\"InstallLocation\":\"{}\"}}",
            vdf_path(&epic_wuwa)
        ),
    );

    let env = FakeEnv {
        steam: Some(steam.clone()),
        app_data: Some(app_data),
        program_data: Some(program_data),
        gryphlink: vec![gryphlink.join("1.2.0"), root.join("NoLauncher")],
    };
    let not_a_game = root.join("Random");
    fs::create_dir_all(&not_a_game).unwrap();
    let manual = vec![("wuwa".to_owned(), not_a_game)];
    let registry = Registry::builtin().unwrap();
    let report = detect(registry, &env, &manual);

    let found: Vec<(&str, InstallSource, String)> = report
        .installs
        .iter()
        .map(|i| {
            let rel = i
                .folder
                .strip_prefix(
                    fs::canonicalize(root)
                        .unwrap()
                        .to_string_lossy()
                        .trim_start_matches(r"\\?\"),
                )
                .map(|p| p.display().to_string().replace('\\', "/"))
                .unwrap_or_else(|_| i.folder.display().to_string());
            (i.game_id.as_str(), i.source, rel)
        })
        .collect();
    assert_eq!(
        found,
        [
            ("wuwa", InstallSource::Steam, "Steam/steamapps/common/Wuthering Waves".to_owned()),
            (
                "zzz",
                InstallSource::Steam,
                "D_drive/SteamLibrary/steamapps/common/Zenless Zone Zero/games/ZenlessZoneZero Game"
                    .to_owned()
            ),
            ("zzz", InstallSource::HoYoPlay, "HoYoverse/ZenlessZoneZero Game".to_owned()),
            (
                "endfield",
                InstallSource::Gryphlink,
                "GRYPHLINK/games/EndField Game".to_owned()
            ),
            ("wuwa", InstallSource::Epic, "Epic/WuWa/Wuthering Waves Game".to_owned()),
        ]
    );
    assert_eq!(report.installs[0].steam_appid, Some(3513350));
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("is not a Wuthering Waves folder")),
        "{:?}",
        report.notes
    );
    assert!(
        !report
            .installs
            .iter()
            .any(|i| i.folder.to_string_lossy().contains("NotInstalled"))
    );
}

#[test]
fn missing_sources_are_quiet() {
    let tmp = tempfile::tempdir().unwrap();
    let env = FakeEnv {
        steam: Some(tmp.path().join("nope")),
        app_data: None,
        program_data: Some(tmp.path().join("also-nope")),
        gryphlink: vec![tmp.path().join("no-launcher")],
    };
    let report = detect(Registry::builtin().unwrap(), &env, &[]);
    assert!(report.installs.is_empty());
    assert!(report.notes.is_empty(), "{:?}", report.notes);
}

#[test]
fn hostile_installdir_is_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let steam = tmp.path().join("Steam");
    write(
        &steam.join("steamapps/appmanifest_3513350.acf"),
        "\"AppState\" { \"installdir\" \"..\\\\..\\\\Windows\" }",
    );
    let env = FakeEnv {
        steam: Some(steam),
        app_data: None,
        program_data: None,
        gryphlink: Vec::new(),
    };
    let report = detect(Registry::builtin().unwrap(), &env, &[]);
    assert!(report.installs.is_empty());
}

#[test]
fn game_folder_markers() {
    let tmp = tempfile::tempdir().unwrap();
    let zzz = Registry::builtin().unwrap().get("zzz").unwrap();
    let folder = tmp.path().join("ZZZ");
    fs::create_dir_all(&folder).unwrap();
    assert!(!is_game_folder(zzz, &folder));
    make_zzz(&folder);
    assert!(is_game_folder(zzz, &folder));
    assert!(same_path(
        Path::new(r"C:\Games\ZZZ\"),
        Path::new(r"c:\games\zzz")
    ));
}
