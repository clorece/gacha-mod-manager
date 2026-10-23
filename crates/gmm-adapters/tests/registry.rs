// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use gmm_adapters::{Engine, Importer, ManifestError, Registry, Tag};

#[test]
fn builtin_manifests_load() {
    let registry = Registry::builtin().unwrap();
    let ids: Vec<_> = registry.games().iter().map(|g| g.id()).collect();
    assert_eq!(ids, ["zzz", "wuwa"]);
}

#[test]
fn zzz_facts() {
    let zzz = Registry::builtin().unwrap().get("zzz").unwrap();
    assert_eq!(zzz.game.name, "Zenless Zone Zero");
    assert_eq!(zzz.game.engine, Engine::Unity);
    assert_eq!(zzz.gamebanana.game_id, 19567);
    assert_eq!(zzz.xxmi.importer, Importer::Zzmi);
    assert_eq!(zzz.xxmi.d3dx_target, "ZenlessZoneZero.exe");
    let steam = zzz.detect.steam.as_ref().unwrap();
    assert_eq!(
        (steam.appid, steam.subpath.as_str()),
        (4162040, "games/ZenlessZoneZero Game")
    );
    assert_eq!(zzz.xxmi_launch_args(), ["--nogui", "--xxmi", "ZZMI"]);
    let skins = zzz.root_category(30305).unwrap();
    assert_eq!(skins.tag, Tag::CharacterSkin);
    assert!(skins.subcategories_are_characters);
}

#[test]
fn wuwa_facts() {
    let wuwa = Registry::builtin().unwrap().get("wuwa").unwrap();
    assert_eq!(wuwa.gamebanana.game_id, 20357);
    assert_eq!(wuwa.xxmi.importer, Importer::Wwmi);
    assert_eq!(wuwa.detect.steam.as_ref().unwrap().appid, 3513350);
    assert_eq!(wuwa.detect.steam.as_ref().unwrap().subpath, "");
    assert_eq!(wuwa.xxmi_launch_args(), ["--nogui", "--xxmi", "WWMI"]);
}

#[test]
fn lookups() {
    let r = Registry::builtin().unwrap();
    assert_eq!(r.get("ZZZ").unwrap().id(), "zzz", "case-insensitive");
    assert_eq!(r.by_gamebanana_id(20357).unwrap().id(), "wuwa");
    assert_eq!(r.by_importer(Importer::Zzmi).unwrap().id(), "zzz");
    assert_eq!(r.by_steam_appid(3513350).unwrap().id(), "wuwa");
    assert!(r.get("genshin").is_none());
    assert!(r.by_importer(Importer::Gimi).is_none());
}

// ---- rejected manifests ---------------------------------------------------------------------

const VALID: &str = r#"
[game]
id = "test"
name = "Test Game"
short_name = "TG"
engine = "unity"
risk_notice = "Use at your own risk."

[gamebanana]
game_id = 1
[[gamebanana.root_categories]]
id = 10
name = "Skins"
tag = "character_skin"

[xxmi]
importer = "GIMI"
d3dx_target = "Game.exe"

[detect]
exe_names = ["Game.exe"]
required_children = ["Game_Data"]

[detect.steam]
appid = 42
subpath = "game"

[[mod_kind]]
id = "migoto"
ini_markers = ["[TextureOverride"]
"#;

fn load(text: &str) -> Result<Registry, ManifestError> {
    Registry::from_sources(&[("test.toml", text)])
}

fn problems(text: &str) -> Vec<String> {
    match load(text) {
        Err(ManifestError::Invalid { problems, .. }) => problems,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn valid_baseline_loads() {
    load(VALID).unwrap();
}

#[test]
fn unknown_fields_and_values_fail_to_parse() {
    let typo = VALID.replace("exe_names", "exe_name");
    assert!(matches!(load(&typo), Err(ManifestError::Parse { .. })));
    let extra = VALID.replace(
        "[detect.steam]",
        "launch_args = [\"--evil\"]\n[detect.steam]",
    );
    assert!(
        matches!(load(&extra), Err(ManifestError::Parse { .. })),
        "no launch args in data"
    );
    let importer = VALID.replace("\"GIMI\"", "\"XYZMI\"");
    assert!(matches!(load(&importer), Err(ManifestError::Parse { .. })));
    let tag = VALID.replace("character_skin", "weapon_skin");
    assert!(matches!(load(&tag), Err(ManifestError::Parse { .. })));
}

#[test]
fn unsafe_paths_rejected() {
    let p = problems(&VALID.replace("subpath = \"game\"", "subpath = \"../../Windows\""));
    assert!(
        p.iter().any(|m| m.contains("detect.steam.subpath")),
        "{p:?}"
    );
    let p = problems(&VALID.replace("[\"Game_Data\"]", "[\"C:/Windows\"]"));
    assert!(p.iter().any(|m| m.contains("required_children")), "{p:?}");
    let p = problems(&VALID.replace(
        "exe_names = [\"Game.exe\"]",
        "exe_names = [\"bin/Game.exe\"]",
    ));
    assert!(p.iter().any(|m| m.contains("exe_names")), "{p:?}");
    let p = problems(&VALID.replace("d3dx_target = \"Game.exe\"", "d3dx_target = \"game.dll\""));
    assert!(p.iter().any(|m| m.contains("d3dx_target")), "{p:?}");
}

#[test]
fn bad_ids_and_empty_lists_rejected() {
    let p = problems(&VALID.replace("id = \"test\"", "id = \"Bad Id\""));
    assert!(p.iter().any(|m| m.contains("game.id")), "{p:?}");
    let p = problems(&VALID.replace("game_id = 1", "game_id = 0"));
    assert!(p.iter().any(|m| m.contains("game_id")), "{p:?}");
    let p = problems(&VALID.replace("id = \"migoto\"", "id = \"pak\""));
    assert!(p.iter().any(|m| m.contains("unknown mod_kind")), "{p:?}");
    let p = problems(&VALID.replace("ini_markers = [\"[TextureOverride\"]", "ini_markers = []"));
    assert!(p.iter().any(|m| m.contains("ini_markers")), "{p:?}");
}

#[test]
fn duplicates_across_manifests_rejected() {
    let other_id = VALID.replace("id = \"test\"", "id = \"other\"");
    // Same GameBanana id, importer and appid as VALID.
    let err = Registry::from_sources(&[("a.toml", VALID), ("b.toml", &other_id)]).unwrap_err();
    assert!(matches!(err, ManifestError::Duplicate(_)), "{err:?}");
    let err = Registry::from_sources(&[("a.toml", VALID), ("b.toml", VALID)]).unwrap_err();
    assert!(err.to_string().contains("game id"), "{err}");
}
