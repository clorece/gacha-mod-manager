// SPDX-License-Identifier: MIT OR Apache-2.0

//! The IPC fuzz harness (docs/security.md, R5): hostile arguments to every command, sent
//! through Tauri's real IPC path (the mock runtime, the app's own context and capability) to
//! the real command list, against a fake XXMI setup in a temporary folder.
//!
//! Each command gets: no arguments, a body that isn't an object, and then each argument in
//! turn replaced by hostile values: wrong JSON types, numbers that overflow or go negative,
//! huge strings, traversal and absolute paths, near misses of a real folder name (case,
//! trailing dot or separator, the `DISABLED ` prefix), unknown games and keys. The network
//! refuses every address ([`Allowlist::deny_all`]), so nothing can reach GameBanana.
//!
//! Checked afterwards: every call answered (no command panicked or hung), nothing on disk
//! changed outside GMM's own folders (the Mods folder, `d3dx_user.ini` and a canary next to
//! it are byte-for-byte the same), path tricks were refused, every command passed the
//! capability (so the arguments reached it) and plugin or unknown commands did not.
//!
//! Left out, each because a valid call opens something on the user's desktop: the folder,
//! archive and log pickers and `open_xxmi_releases` (native dialogs; they take no arguments),
//! and `clear_xxmi_folder` (it would point GMM back at
//! the real XXMI), and the window commands (they act on the calling window itself and take
//! no arguments); `launch`, `pick_game_folder`, `open_mod_page`, `open_bug_report` and
//! `open_app_release` only
//! get arguments that must be refused before anything opens.

// The harness builds and inspects its fake world directly; GMM itself still goes through
// gmm-fsguard.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use gmm_core::AppDirs;
use gmm_net::{Allowlist, NetConfig};
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{AppState, handler};

/// What an argument is, which picks its hostile values.
#[derive(Clone, Copy)]
enum Kind {
    /// A game id.
    Game,
    /// A GameBanana id (u64).
    GbId,
    /// A GMM row id, job or profile id (i64 / u64).
    Id,
    /// A `ModKey`.
    Key,
    /// A list of `ModKey`s.
    Keys,
    /// Free text: search text, profile names, share codes, link keys.
    Text,
    /// Something that looks like a path but is only compared (adopt, remove_game_folder).
    Path,
    Bool,
    /// A `Channel` (the webview passes `__CHANNEL__:<id>`).
    Channel,
    /// A struct argument (`InstallArgs`, `CatalogQuery`, `Preferences`, `VariantPick`).
    Object,
    /// A page number (u32).
    Page,
}

struct Spec {
    cmd: &'static str,
    /// Arguments that are harmless on their own (unknown ids, no real folder).
    base: Value,
    fields: &'static [(&'static str, Kind)],
}

fn specs() -> Vec<Spec> {
    use Kind::*;
    let s = |cmd, base, fields| Spec { cmd, base, fields };
    let missing = json!({"type": "managed", "id": 999_999});
    vec![
        s("games", json!({}), &[]),
        s("statuses", json!({}), &[]),
        s(
            "catalog_list",
            json!({"query": {"game": "zzz", "page": 1, "per_page": 30, "sort": "default",
                   "categories": [], "text": null, "show_nsfw": false}}),
            &[("query", Object)],
        ),
        s(
            "catalog_search",
            json!({"game": "zzz", "text": "x", "page": 1, "showNsfw": false}),
            &[
                ("game", Game),
                ("text", Text),
                ("page", Page),
                ("showNsfw", Bool),
            ],
        ),
        s("catalog_mod", json!({"modId": 1}), &[("modId", GbId)]),
        s(
            "catalog_updates",
            json!({"modId": 1, "page": 1}),
            &[("modId", GbId), ("page", Page)],
        ),
        s(
            "catalog_categories",
            json!({"game": "zzz", "parent": null}),
            &[("game", Game), ("parent", GbId)],
        ),
        s(
            "catalog_featured",
            json!({"game": "zzz", "showNsfw": false}),
            &[("game", Game), ("showNsfw", Bool)],
        ),
        s(
            "catalog_stats",
            json!({"modIds": [1], "onStats": "nope"}),
            &[("modIds", Keys), ("onStats", Channel)],
        ),
        s(
            "install",
            json!({"args": {}, "onProgress": "nope"}),
            &[("args", Object), ("onProgress", Channel)],
        ),
        s(
            "queue_install",
            json!({"args": {"mod_id": 1, "file_id": null, "variants": {"type": "only"},
                   "skip_blocked": false, "start_disabled": true}}),
            &[("args", Object)],
        ),
        s(
            "queue_change_file",
            json!({"gbModId": 1, "fileId": null}),
            &[("gbModId", GbId), ("fileId", GbId)],
        ),
        s("app_update_check", json!({}), &[]),
        // Only the version the last check found opens or installs anything; the checks here
        // never find one.
        s(
            "open_app_release",
            json!({"version": "9.9.9"}),
            &[("version", Text)],
        ),
        s(
            "app_update_install",
            json!({"version": "9.9.9", "onProgress": "nope"}),
            &[("version", Text), ("onProgress", Channel)],
        ),
        s("queue_jobs", json!({}), &[]),
        s(
            "queue_watch",
            json!({"onChange": "nope"}),
            &[("onChange", Channel)],
        ),
        s(
            "queue_proceed",
            json!({"id": 999_999, "variants": null}),
            &[("id", Id), ("variants", Object)],
        ),
        s("queue_pause", json!({"id": 999_999}), &[("id", Id)]),
        s("queue_resume", json!({"id": 999_999}), &[("id", Id)]),
        s("queue_cancel", json!({"id": 999_999}), &[("id", Id)]),
        s("queue_dismiss", json!({"id": 999_999}), &[("id", Id)]),
        s("queue_clear_finished", json!({}), &[]),
        s(
            "startup_check",
            json!({"onStep": "nope"}),
            &[("onStep", Channel)],
        ),
        s("library", json!({"game": "nope"}), &[("game", Game)]),
        s(
            "switch_version",
            json!({"game": "zzz", "id": 999_999}),
            &[("game", Game), ("id", Id)],
        ),
        s(
            "rollback_files",
            json!({"game": "zzz", "gbModId": 1}),
            &[("game", Game), ("gbModId", GbId)],
        ),
        s(
            "set_enabled",
            json!({"game": "zzz", "key": missing, "enabled": false, "anyway": false}),
            &[
                ("game", Game),
                ("key", Key),
                ("enabled", Bool),
                ("anyway", Bool),
            ],
        ),
        s(
            "uninstall_preview",
            json!({"game": "zzz", "keys": [missing]}),
            &[("game", Game), ("keys", Keys)],
        ),
        s(
            "uninstall",
            json!({"game": "zzz", "keys": [missing]}),
            &[("game", Game), ("keys", Keys)],
        ),
        s(
            "mod_keybindings",
            json!({"game": "zzz", "key": missing}),
            &[("game", Game), ("key", Key)],
        ),
        s(
            "adopt_candidates",
            json!({"game": "zzz", "path": "nope"}),
            &[("game", Game), ("path", Path)],
        ),
        s(
            "adopt",
            json!({"game": "zzz", "path": "nope", "gbModId": 1, "fileId": null}),
            &[
                ("game", Game),
                ("path", Path),
                ("gbModId", GbId),
                ("fileId", GbId),
            ],
        ),
        s(
            "drop_watch",
            json!({"onDrop": "nope"}),
            &[("onDrop", Channel)],
        ),
        s("prepare_local", json!({"game": "zzz"}), &[("game", Game)]),
        s(
            "install_local",
            json!({"variants": {"type": "only"}, "skipBlocked": false, "startDisabled": true}),
            &[
                ("variants", Object),
                ("skipBlocked", Bool),
                ("startDisabled", Bool),
            ],
        ),
        s("discard_local", json!({}), &[]),
        s("conflicts", json!({"game": "nope"}), &[("game", Game)]),
        s("storage", json!({}), &[]),
        s("clear_download_cache", json!({}), &[]),
        s("clear_http_cache", json!({}), &[]),
        s("clear_image_cache", json!({}), &[]),
        s("network_status", json!({}), &[]),
        s("report_log", json!({}), &[]),
        s(
            "log_ui_error",
            json!({"message": "x"}),
            &[("message", Text)],
        ),
        // A valid call opens the confirmation dialog: only shapes that are refused first.
        s("open_bug_report", json!({"title": 1}), &[]),
        s("launch_plan", json!({"game": "nope"}), &[("game", Game)]),
        s(
            "launch",
            json!({"game": "nope", "confirm": true}),
            &[("game", Game), ("confirm", Bool)],
        ),
        s("settings", json!({}), &[]),
        s("preferences", json!({}), &[]),
        s(
            "set_preferences",
            json!({"preferences": {"theme": "dark", "show_sensitive": false,
                   "blur_sensitive": true, "onboarded": true}}),
            &[("preferences", Object)],
        ),
        s(
            "pick_game_folder",
            json!({"game": "nope"}),
            &[("game", Game)],
        ),
        s(
            "remove_game_folder",
            json!({"game": "zzz", "folder": "nope"}),
            &[("game", Game), ("folder", Path)],
        ),
        s("open_mod_page", json!({"modId": "nope"}), &[]),
        s(
            "open_link",
            json!({"key": "0".repeat(32)}),
            &[("key", Text)],
        ),
        s(
            "open_mod_folder",
            json!({"game": "zzz", "key": missing}),
            &[("game", Game), ("key", Key)],
        ),
        s("profiles", json!({"game": "nope"}), &[("game", Game)]),
        s(
            "profile_save",
            json!({"game": "nope", "name": "x"}),
            &[("game", Game), ("name", Text)],
        ),
        s(
            "profile_apply",
            json!({"game": "zzz", "id": 999_999}),
            &[("game", Game), ("id", Id)],
        ),
        s(
            "profile_rename",
            json!({"game": "zzz", "id": 999_999, "name": "x"}),
            &[("game", Game), ("id", Id), ("name", Text)],
        ),
        s(
            "profile_delete",
            json!({"game": "zzz", "id": 999_999}),
            &[("game", Game), ("id", Id)],
        ),
        s(
            "profile_export",
            json!({"game": "zzz", "id": 999_999}),
            &[("game", Game), ("id", Id)],
        ),
        s(
            "profile_import",
            json!({"game": "zzz", "code": "nope", "name": null}),
            &[("game", Game), ("code", Text), ("name", Text)],
        ),
    ]
}

/// Commands the harness never runs (see the module docs).
const LEFT_OUT: &[&str] = &[
    "pick_xxmi_folder",
    "pick_archive",
    "pick_report_log",
    "open_xxmi_releases",
    "clear_xxmi_folder",
    "window_minimize",
    "window_toggle_maximize",
    "window_is_maximized",
    "window_close",
    "window_start_drag",
];

/// JSON values of the wrong type for anything.
fn wrong_types() -> Vec<Value> {
    vec![
        Value::Null,
        json!(true),
        json!(-1),
        json!(1.5),
        json!([]),
        json!({}),
        json!({"__proto__": {"polluted": true}}),
    ]
}

fn huge() -> String {
    "A".repeat(256 * 1024)
}

/// Strings that look like paths and must never name anything outside the Mods folder.
fn paths(world: &World) -> Vec<String> {
    let outside = world.outside.to_string_lossy().into_owned();
    vec![
        String::new(),
        "..".to_owned(),
        "../outside".to_owned(),
        "..\\outside".to_owned(),
        "..\\..\\..\\outside".to_owned(),
        "Manual\\..\\..\\outside".to_owned(),
        "Manual/../../outside".to_owned(),
        outside.clone(),
        format!("\\\\?\\{outside}"),
        "C:\\Windows\\System32".to_owned(),
        "/etc/passwd".to_owned(),
        "%APPDATA%\\GMM".to_owned(),
        // Near misses of the real "Manual" folder: refused, never matched loosely.
        "MANUAL".to_owned(),
        "Manual.".to_owned(),
        "Manual\\".to_owned(),
        " Manual".to_owned(),
        "DISABLED Manual".to_owned(),
        "Manual\u{0}".to_owned(),
        huge(),
    ]
}

fn hostile(kind: Kind, world: &World) -> Vec<Value> {
    let mut out = wrong_types();
    match kind {
        Kind::Game => {
            // Game ids match case-insensitively ("ZZZ" is ZZZ), so case variants aren't hostile.
            out.extend(["", "zzz ", "zzz\u{0}", "../zzz", "all", "\u{202e}zzz", "<script>"].map(|s| json!(s)));
            out.push(json!(huge()));
        }
        Kind::GbId | Kind::Id | Kind::Page => {
            let overflow: Value = serde_json::from_str("18446744073709551616").unwrap_or(Value::Null);
            out.extend([json!("1"), json!(1e300), json!(i64::MIN), overflow, json!(0), json!(999_999)]);
            if !matches!(kind, Kind::Page) {
                out.push(json!(u64::MAX));
            }
        }
        Kind::Key => {
            out.extend([
                json!({"type": "managed"}),
                json!({"type": "managed", "id": "1"}),
                json!({"type": "managed", "id": 999_999}),
                json!({"type": "managed", "id": i64::MIN}),
                json!({"type": "unmanaged"}),
                json!({"type": "evil", "path": "Manual"}),
                json!("Manual"),
            ]);
            out.extend(paths(world).into_iter().map(|p| json!({"type": "unmanaged", "path": p})));
        }
        Kind::Keys => {
            out.push(json!(["Manual"]));
            out.push(json!(vec![json!({"type": "managed", "id": 999_999}); 10_000]));
            out.extend(paths(world).into_iter().map(|p| json!([{"type": "unmanaged", "path": p}])));
        }
        Kind::Text => {
            out.extend([
                json!(""),
                json!(huge()),
                json!("\u{0}"),
                json!("<img src=x onerror=alert(1)>"),
                json!("{\"version\":1,\"game\":\"zzz\",\"mods\":[".repeat(2000)),
                json!(format!("{{\"version\":99,\"game\":\"zzz\",\"name\":\"{}\",\"mods\":[]}}", huge())),
                json!("{\"version\":1,\"game\":\"wuwa\",\"name\":\"x\",\"mods\":[]}"),
            ]);
        }
        Kind::Path => out.extend(paths(world).into_iter().map(Value::String)),
        Kind::Bool => out.extend([json!("true"), json!(1), json!(0)]),
        Kind::Channel => out.extend([json!("nope"), json!("__CHANNEL__:"), json!("__CHANNEL__:x"), json!(huge())]),
        Kind::Object => out.extend([
            json!({"mod_id": -1}),
            json!({"mod_id": "1", "file_id": "2", "variants": {"type": "all"}}),
            json!({"type": "named", "names": ["../../outside"]}),
            json!({"type": "named", "names": [huge()]}),
            json!({"game": "../zzz", "page": 0, "per_page": u32::MAX, "sort": "evil", "categories": [-1],
                   "text": huge(), "show_nsfw": "yes"}),
            json!({"theme": "neon", "show_sensitive": 1, "blur_sensitive": null, "onboarded": "yes"}),
            json!(nested(500)),
        ]),
    }
    out
}

/// `[[[[...]]]]`, deeper than serde_json's recursion limit.
fn nested(depth: usize) -> String {
    format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

struct World {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    dirs: AppDirs,
    xxmi: PathBuf,
    mods: PathBuf,
    outside: PathBuf,
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("create dirs");
    fs::write(path, bytes).expect("write");
}

impl World {
    /// A game folder, an XXMI root with ZZMI and one hand-installed mod, and a canary folder
    /// beside the Mods folder that nothing may touch.
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let canonical = fs::canonicalize(tmp.path()).expect("canonical");
        let root = PathBuf::from(canonical.to_string_lossy().trim_start_matches(r"\\?\"));
        let game = root.join("Games/ZenlessZoneZero Game");
        write(&game.join("ZenlessZoneZero.exe"), b"MZ game");
        fs::create_dir_all(game.join("ZenlessZoneZero_Data")).expect("data dir");
        let xxmi = root.join("XXMI Launcher");
        write(&xxmi.join("Resources/Bin/XXMI Launcher.exe"), b"MZ\x90\x00");
        let game_json = game.to_string_lossy().replace('\\', "\\\\");
        write(
            &xxmi.join("XXMI Launcher Config.json"),
            format!(
                r#"{{ "Launcher": {{ "config_version": "2.4.9" }},
                     "Importers": {{ "ZZMI": {{ "Importer": {{ "game_folder": "{game_json}",
                       "game_launch": "DIRECT" }} }} }} }}"#
            )
            .as_bytes(),
        );
        let zzmi = xxmi.join("ZZMI");
        write(&zzmi.join("d3d11.dll"), b"not a real dll");
        write(
            &zzmi.join("d3dx.ini"),
            b"[Loader]\ntarget = ZenlessZoneZero.exe\n[Include]\ninclude = Core\\ZZMI\\main.ini\n\
              include_recursive = Mods\nexclude_recursive = DISABLED*\n",
        );
        write(&zzmi.join("Core/ZZMI/main.ini"), b"global $version = 1.0\n");
        write(
            &zzmi.join("d3dx_user.ini"),
            b"$\\mods\\manual\\manual.ini\\swap = 1\n",
        );
        let mods = zzmi.join("Mods");
        write(
            &mods.join("Manual/manual.ini"),
            b"[TextureOverrideManual]\nhash = 11111111\n",
        );
        let outside = zzmi.join("outside");
        write(&outside.join("secret.txt"), b"canary");
        let dirs = AppDirs {
            data: root.join("gmm/data"),
            cache: root.join("gmm/cache"),
        };
        Self {
            _tmp: tmp,
            root,
            dirs,
            xxmi,
            mods,
            outside,
        }
    }

    /// Every file outside GMM's own folders, with its bytes.
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(dir: &Path, skip: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(dir).expect("read_dir").flatten() {
                let path = entry.path();
                if path.starts_with(skip) {
                    continue;
                }
                if path.is_dir() {
                    out.insert(path.join("<dir>"), Vec::new());
                    walk(&path, skip, out);
                } else {
                    out.insert(path.clone(), fs::read(&path).expect("read"));
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.root, &self.root.join("gmm"), &mut out);
        out
    }
}

fn request(cmd: &str, body: InvokeBody) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        }
        .parse()
        .expect("url"),
        body,
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_owned(),
    }
}

fn is_acl_denial(err: &Value) -> bool {
    let text = err.to_string();
    text.contains("not allowed") || text.contains("not found")
}

#[test]
fn hostile_arguments_to_every_command() {
    let world = World::new();
    let state = tauri::async_runtime::block_on(async {
        let state = AppState::open_with(
            world.dirs.clone(),
            NetConfig {
                allowlist: Allowlist::deny_all(),
                ..NetConfig::production()
            },
        )
        .await
        .expect("app state");
        state
            .db
            .set_xxmi_root(Some(&world.xxmi))
            .await
            .expect("xxmi root");
        state
    });
    let app = mock_builder()
        .invoke_handler(handler())
        .build(tauri::generate_context!())
        .expect("mock app");
    app.manage(state);
    let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("webview");
    let call =
        |cmd: &str, body: Value| get_ipc_response(&webview, request(cmd, InvokeBody::Json(body)));

    // The fake setup must be ready, or the game commands would stop before the checks that
    // matter (listing a folder, matching a path).
    let lib = call("library", json!({"game": "zzz"}))
        .expect("library")
        .deserialize::<Value>()
        .expect("json");
    assert_eq!(
        lib["problem"],
        Value::Null,
        "the fake XXMI setup isn't ready: {lib}"
    );
    assert_eq!(lib["unmanaged"][0]["path"], json!("Manual"), "{lib}");

    let before = world.snapshot();
    let specs = specs();
    let mut sent = 0usize;
    for spec in &specs {
        assert!(!LEFT_OUT.contains(&spec.cmd));
        // The capability lets every command through: its argument errors come from the
        // command, not from the ACL.
        if let Err(e) = call(spec.cmd, json!({"__unexpected": nested(10)})) {
            assert!(
                !is_acl_denial(&e) || spec.fields.is_empty(),
                "{}: refused by the ACL: {e}",
                spec.cmd
            );
        }
        for body in [json!([1, 2]), json!("x"), Value::Null, json!(nested(200))] {
            let _ = call(spec.cmd, body);
            sent += 1;
        }
        for (field, kind) in spec.fields {
            for value in hostile(*kind, &world) {
                let mut body = spec.base.clone();
                body[*field] = value.clone();
                let result = call(spec.cmd, body);
                sent += 1;
                // Anything that names a folder by a near miss or a path trick is refused.
                if matches!(kind, Kind::Key | Kind::Keys | Kind::Path)
                    && value.to_string().contains("outside")
                {
                    assert!(result.is_err(), "{} accepted {field} = {value}", spec.cmd);
                }
            }
        }
    }
    assert!(sent > 1_000, "only {sent} calls");

    // Every command the app registers is covered or deliberately left out.
    let build_rs = include_str!("../build.rs");
    for line in build_rs.lines().map(str::trim) {
        if let Some(cmd) = line.strip_prefix('"').and_then(|l| l.strip_suffix("\",")) {
            assert!(
                specs.iter().any(|s| s.cmd == cmd) || LEFT_OUT.contains(&cmd),
                "{cmd} isn't in the fuzz harness"
            );
        }
    }

    // Nothing outside GMM's own folders changed: not the Mods folder, not the saved
    // toggles, not the canary next to them.
    let after = world.snapshot();
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    assert!(before == after, "a file outside GMM's folders changed");
    assert!(world.mods.join("Manual/manual.ini").exists());

    // The capability refuses plugin commands and unknown ones.
    for cmd in [
        "plugin:dialog|open",
        "plugin:fs|read_text_file",
        "plugin:shell|open",
        "plugin:event|listen",
        "plugin:window|close",
        "plugin:opener|open_url",
        "evil_command",
    ] {
        let err = call(
            cmd,
            json!({"path": "C:\\Windows\\win.ini", "url": "https://evil.example"}),
        )
        .expect_err(cmd);
        assert!(is_acl_denial(&err), "{cmd}: {err}");
    }
}
