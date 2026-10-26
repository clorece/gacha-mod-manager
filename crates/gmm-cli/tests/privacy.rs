// SPDX-License-Identifier: MIT OR Apache-2.0

//! Privacy harnesses (docs/04-security.md, R2–R4), run against the real `gmm` binary:
//!
//! - **Egress:** every command runs behind a logging proxy that refuses all requests. Every
//!   host GMM tried to reach must be on the compiled-in allowlist, and commands that work
//!   locally must not try to reach anything.
//! - **File access:** a fake profile full of canary files (Steam login data, a game log, a
//!   browser profile, an SSH key, secrets in XXMI's config). After the run, nothing in the
//!   profile outside the game's `Mods/` folder has changed, and no canary content shows up
//!   in GMM's own files.
//!
//! With `GMM_PRIVACY_STRACE=1` on Linux (CI does this), every command also runs under
//! `strace`, and the test fails if `gmm` connects anywhere but loopback (traffic that
//! bypasses the proxy), opens or stats any canary file, or writes outside its own folders and
//! `Mods/`.

#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::unwrap_used,
    clippy::expect_used
)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use gmm_net::{Allowlist, Url};

const GMM: &str = env!("CARGO_BIN_EXE_gmm");

/// Canary files: (path relative to the fake profile, content). GMM must never open these.
const CANARY_FILES: &[(&str, &str)] = &[
    (
        ".steam/steam/config/loginusers.vdf",
        "\"users\" { \"CANARY_STEAM_LOGIN\" { } }",
    ),
    (".steam/steam/config/config.vdf", "CANARY_STEAM_CONFIG"),
    (
        ".steam/steam/userdata/12345/config/localconfig.vdf",
        "CANARY_STEAM_USERDATA",
    ),
    (
        "Games/ZenlessZoneZero Game/ZenlessZoneZero_Data/Player.log",
        "CANARY_GAME_LOG",
    ),
    (
        ".mozilla/firefox/abc.default/logins.json",
        "CANARY_BROWSER_LOGINS",
    ),
    (".ssh/id_rsa", "CANARY_SSH_KEY"),
];
/// Secrets inside XXMI's config: GMM may open that file but must keep none of these.
const CONFIG_SECRETS: &[&str] = &["ghp_CANARY", "CANARY_PW", "CANARY_SIG", "CANARY_USER_SIG"];
/// What strace lines must never mention.
const CANARY_NAMES: &[&str] = &[
    "loginusers.vdf",
    "/config/config.vdf",
    "localconfig.vdf",
    "/userdata",
    "Player.log",
    "logins.json",
    "id_rsa",
];
/// The hand-installed mod's saved in-game toggle in `d3dx_user.ini`.
const MANUAL_KEY: &str = "$\\mods\\manual\\manual.ini\\v = 3\r\n";
/// 3DMigoto's `d3dx_user.ini` as the harness starts it: XXMI's own key and the mod's.
const USER_INI: &str = "; AUTOMATICALLY GENERATED FILE - DO NOT EDIT\r\n[Constants]\r\n\
    $\\zzmiv1\\shown = 1\r\n$\\mods\\manual\\manual.ini\\v = 3\r\n";

// ---- a refusing, logging proxy ---------------------------------------------------------

struct Proxy {
    url: String,
    log: Arc<Mutex<Vec<String>>>,
}

impl Proxy {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let shared = Arc::clone(&log);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = Arc::clone(&shared);
                thread::spawn(move || refuse(stream, &log));
            }
        });
        Self { url, log }
    }

    fn targets(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

/// Records the request line (`CONNECT host:443 HTTP/1.1`) and answers 403.
fn refuse(mut stream: TcpStream, log: &Mutex<Vec<String>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < 16 * 1024 {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => head.extend_from_slice(&buf[..n]),
        }
    }
    let text = String::from_utf8_lossy(&head);
    if let Some(line) = text.lines().next().filter(|l| !l.is_empty()) {
        log.lock().unwrap().push(line.to_owned());
    }
    let _ = stream
        .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
}

// ---- the fake profile --------------------------------------------------------------------

struct World {
    _tmp: tempfile::TempDir,
    profile: PathBuf,
    gmm_home: PathBuf,
    xxmi: PathBuf,
    game: PathBuf,
    mods: PathBuf,
    /// 3DMigoto's saved toggles: the one file outside Mods/ GMM may write (R3).
    user_ini: PathBuf,
    traces: PathBuf,
    proxy: Proxy,
    strace: bool,
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

impl World {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        // Canonical (without Windows' `\\?\` prefix), so strace paths and ours compare equal.
        let canonical = fs::canonicalize(tmp.path()).unwrap();
        let base = PathBuf::from(canonical.to_string_lossy().trim_start_matches(r"\\?\"));
        let profile = base.join("profile");
        let game = profile.join("Games/ZenlessZoneZero Game");
        write(&game.join("ZenlessZoneZero.exe"), b"MZ game");
        fs::create_dir_all(game.join("ZenlessZoneZero_Data")).unwrap();
        for (rel, content) in CANARY_FILES {
            write(&profile.join(rel), content.as_bytes());
        }
        // A Steam library that lists no games (an allowed read).
        write(
            &profile.join(".steam/steam/steamapps/libraryfolders.vdf"),
            b"\"libraryfolders\" { }",
        );

        let xxmi = profile.join(".config/XXMI Launcher");
        write(&xxmi.join("Resources/Bin/XXMI Launcher.exe"), b"MZ\x90\x00");
        let game_json = game.to_string_lossy().replace('\\', "\\\\");
        write(
            &xxmi.join("XXMI Launcher Config.json"),
            format!(
                r#"{{ "Launcher": {{ "config_version": "2.4.9", "github_token": "ghp_CANARY",
                       "proxy": {{ "password": "CANARY_PW" }} }},
                     "Importers": {{ "ZZMI": {{ "Importer": {{ "game_folder": "{game_json}",
                       "game_launch": "DIRECT", "run_pre_launch_signature": "CANARY_SIG" }} }} }},
                     "Security": {{ "user_signature": "CANARY_USER_SIG" }} }}"#
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
        let mods = zzmi.join("Mods");
        write(
            &mods.join("Manual/manual.ini"),
            b"[TextureOverrideManual]\nhash = 11111111\nib = R\n",
        );
        let user_ini = zzmi.join("d3dx_user.ini");
        write(&user_ini, USER_INI.as_bytes());

        let traces = base.join("traces");
        fs::create_dir_all(&traces).unwrap();
        fs::create_dir_all(base.join("gmm-home/tmp")).unwrap();
        let strace = cfg!(target_os = "linux") && std::env::var_os("GMM_PRIVACY_STRACE").is_some();
        Self {
            gmm_home: base.join("gmm-home"),
            _tmp: tmp,
            profile,
            xxmi,
            game,
            mods,
            user_ini,
            traces,
            proxy: Proxy::start(),
            strace,
        }
    }

    /// `gmm <args>`, isolated: GMM's folders under `gmm_home`, the profile as home (Linux),
    /// all traffic sent to the refusing proxy, optionally under strace.
    fn command(&self, name: &str, args: &[&str]) -> Command {
        let mut cmd = if self.strace {
            let mut c = Command::new("strace");
            c.args(["-f", "-qq", "-e", "trace=%file,connect", "-o"])
                .arg(self.traces.join(format!("{name}.strace")))
                .arg(GMM);
            c
        } else {
            Command::new(GMM)
        };
        cmd.args(args)
            .env("GMM_HOME", &self.gmm_home)
            // SQLite's temporary files, if it ever needs any.
            .env("TMPDIR", self.gmm_home.join("tmp"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for var in [
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ] {
            cmd.env(var, &self.proxy.url);
        }
        cmd.env_remove("NO_PROXY").env_remove("no_proxy");
        if !cfg!(windows) {
            cmd.env("HOME", &self.profile)
                .env("XDG_CONFIG_HOME", self.profile.join(".config"))
                .env("XDG_DATA_HOME", self.profile.join(".local/share"))
                .env("XDG_CACHE_HOME", self.profile.join(".cache"));
        }
        cmd
    }

    fn run(&self, name: &str, args: &[&str]) -> Output {
        self.command(name, args).output().unwrap()
    }

    fn xxmi_arg(&self) -> String {
        self.xxmi.to_string_lossy().into_owned()
    }

    fn folder_arg(&self) -> String {
        format!("zzz={}", self.game.to_string_lossy())
    }
}

/// Every file and folder below `dir` except under `skip`, with size and mtime.
fn snapshot(dir: &Path, skip: &[&Path]) -> BTreeMap<PathBuf, (u64, Option<SystemTime>)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if skip.iter().any(|s| path.starts_with(s)) {
                continue;
            }
            let meta = fs::symlink_metadata(&path).unwrap();
            out.insert(path.clone(), (meta.len(), meta.modified().ok()));
            if meta.is_dir() {
                stack.push(path);
            }
        }
    }
    out
}

fn files_below(dir: &Path) -> Vec<PathBuf> {
    snapshot(dir, &[])
        .into_keys()
        .filter(|p| p.is_file())
        .collect()
}

// ---- the scenario ----------------------------------------------------------------------

#[test]
fn egress_and_file_access() {
    let w = World::new();
    let before = snapshot(&w.profile, &[&w.mods]);
    let xxmi = w.xxmi_arg();
    let folder = w.folder_arg();

    // Commands that talk to GameBanana: started together, since each one retries a refused
    // request a few times before giving up.
    let network: Vec<(&str, Vec<&str>)> = vec![
        (
            "catalog-list",
            vec!["catalog", "list", "--game", "zzz", "--per-page", "2"],
        ),
        (
            "catalog-search",
            vec!["catalog", "search", "--game", "wuwa", "hair"],
        ),
        ("catalog-show", vec!["catalog", "show", "1"]),
        (
            "catalog-categories",
            vec!["catalog", "categories", "--game", "zzz"],
        ),
        ("fetch", vec!["fetch", "1"]),
        (
            "install",
            vec!["install", "1", "--game", "zzz", "--xxmi", &xxmi],
        ),
        ("update", vec!["update", "1", "--xxmi", &xxmi]),
        ("rollback", vec!["rollback", "1", "--xxmi", &xxmi]),
    ];
    let children: Vec<(&str, Child)> = network
        .iter()
        .map(|(name, args)| (*name, w.command(name, args).spawn().unwrap()))
        .collect();
    for (name, child) in children {
        let out = child.wait_with_output().unwrap();
        assert!(
            !out.status.success(),
            "{name} can't succeed without the network: {}",
            String::from_utf8_lossy(&out.stdout)
        );
    }
    let targets = w.proxy.targets();
    assert!(
        !targets.is_empty(),
        "no request reached the proxy, so the harness saw nothing"
    );
    let allowlist = Allowlist::production();
    for line in &targets {
        // `CONNECT host:port HTTP/1.1` for HTTPS through a proxy.
        let target = line
            .strip_prefix("CONNECT ")
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or_else(|| panic!("not an HTTPS tunnel request: {line:?}"));
        let url = Url::parse(&format!("https://{target}/")).unwrap();
        assert!(
            allowlist.check(&url).is_ok() && url.port_or_known_default() == Some(443),
            "GMM tried to reach a host that isn't allowed: {target}"
        );
    }

    // Commands that work locally: no network at all.
    let seen = targets.len();
    let local: Vec<(&str, Vec<&str>)> = vec![
        ("games", vec!["games"]),
        (
            "detect",
            vec!["detect", "--xxmi", &xxmi, "--folder", &folder],
        ),
        (
            "status",
            vec!["status", "--xxmi", &xxmi, "--folder", &folder],
        ),
        ("config-add", vec!["config", "add-folder", &folder]),
        ("config-xxmi", vec!["config", "set-xxmi", &xxmi]),
        ("config-show", vec!["config", "show"]),
        ("list", vec!["list"]),
        ("conflicts", vec!["conflicts"]),
        // Switching off saves the mod's toggles; switching on writes back what the game
        // dropped in between (the loop below drops it before "enable").
        ("disable", vec!["disable", "Manual", "--game", "zzz"]),
        ("enable", vec!["enable", "Manual", "--game", "zzz"]),
        ("storage", vec!["storage"]),
        // Nothing installed, so there is nothing to check.
        ("update-check", vec!["update"]),
        (
            "profile-save",
            vec!["profile", "save", "zzz", "Test", "--xxmi", &xxmi],
        ),
        ("profile-list", vec!["profile", "list", "zzz"]),
        ("profile-export", vec!["profile", "export", "zzz", "Test"]),
        ("launch", vec!["launch", "zzz"]),
        // Bug reports only build a link; the log holds launch's refusal by now.
        (
            "report-bug",
            vec![
                "report-bug",
                "--title",
                "t",
                "--xxmi",
                &xxmi,
                "--folder",
                &folder,
            ],
        ),
        ("log", vec!["log"]),
        ("config-remove", vec!["config", "remove-folder", &folder]),
        ("config-clear", vec!["config", "clear-xxmi"]),
    ];
    for (name, args) in &local {
        if *name == "enable" {
            // What 3DMigoto does when it saves while the mod is off.
            fs::write(&w.user_ini, USER_INI.replace(MANUAL_KEY, "")).unwrap();
        }
        let out = w.run(name, args);
        let expect_ok = *name != "launch";
        assert_eq!(
            out.status.success(),
            expect_ok,
            "{name}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let status = String::from_utf8_lossy(
        &w.run(
            "status-again",
            &["status", "--xxmi", &xxmi, "--folder", &folder],
        )
        .stdout,
    )
    .into_owned();
    assert!(status.contains("ready"), "{status}");
    assert_eq!(
        w.proxy.targets().len(),
        seen,
        "a local command made a network request: {:?}",
        &w.proxy.targets()[seen..]
    );

    // Nothing in the profile changed except under Mods/ (a mod switched off and on again; no
    // install succeeded) and d3dx_user.ini, which holds the mod's toggle again.
    // (The importer folder's own entry changes too: a file inside it was replaced.)
    let importer_dir = w.user_ini.parent().unwrap();
    let mut expected = before;
    expected.remove(&w.user_ini);
    expected.remove(importer_dir);
    let mut after = snapshot(&w.profile, &[&w.mods, &w.user_ini]);
    after.remove(importer_dir);
    assert_eq!(after, expected, "a file outside GMM's folders changed");
    // GMM never writes the file while the game runs; on a machine where ZZZ is open the
    // write-back waits (and the file must be exactly as the "game" left it).
    let zzz = gmm_adapters::Registry::builtin()
        .unwrap()
        .get("zzz")
        .unwrap();
    let user_ini = fs::read_to_string(&w.user_ini).unwrap();
    if gmm_xxmi::game_running(zzz) {
        eprintln!("note: Zenless Zone Zero is running here, so the toggle write-back waited");
        assert_eq!(user_ini, USER_INI.replace(MANUAL_KEY, ""));
    } else {
        assert_eq!(user_ini, USER_INI);
    }

    // No canary content in GMM's own files.
    for file in files_below(&w.gmm_home) {
        let text = String::from_utf8_lossy(&fs::read(&file).unwrap()).into_owned();
        for (_, canary) in CANARY_FILES {
            assert!(!text.contains(canary), "{canary} found in {file:?}");
        }
        for secret in CONFIG_SECRETS {
            assert!(!text.contains(secret), "{secret} found in {file:?}");
        }
    }

    if w.strace {
        check_traces(&w);
    } else {
        eprintln!("note: syscall tracing is off (set GMM_PRIVACY_STRACE=1 on Linux with strace)");
    }
}

// ---- strace checks -----------------------------------------------------------------------

fn check_traces(w: &World) {
    let allowed_write_roots = [w.gmm_home.clone(), w.mods.clone()];
    // d3dx_user.ini, and the temporary file it is written through (`.d3dx_user.ini.gmm-tmp-*`).
    let user_ini_dir = w.user_ini.parent().unwrap();
    let is_user_ini = |path: &Path| {
        path.parent() == Some(user_ini_dir)
            && path.file_name().is_some_and(|n| {
                let n = n.to_string_lossy();
                n == "d3dx_user.ini" || n.starts_with(".d3dx_user.ini.gmm-tmp-")
            })
    };
    let mut checked = 0;
    for trace in files_below(&w.traces) {
        let text = fs::read_to_string(&trace).unwrap();
        for line in text.lines() {
            checked += 1;
            check_connect(line, &trace);
            for name in CANARY_NAMES {
                assert!(
                    !line.contains(name),
                    "{}: GMM touched a canary file: {line}",
                    trace.display()
                );
            }
            if is_write(line) {
                for path in quoted(line) {
                    let ok = path.starts_with("/dev/")
                        || path.starts_with("/proc/")
                        || is_user_ini(Path::new(&path))
                        || allowed_write_roots
                            .iter()
                            .any(|r| Path::new(&path).starts_with(r));
                    assert!(
                        ok,
                        "{}: write outside GMM's folders: {line}",
                        trace.display()
                    );
                }
            }
        }
    }
    assert!(checked > 0, "strace produced no output");
}

/// IPv4/IPv6 connections must go to loopback (the proxy). Unix sockets are fine.
fn check_connect(line: &str, trace: &Path) {
    if !line.contains("connect(") {
        return;
    }
    let addr = if let Some(rest) = line.split("inet_addr(\"").nth(1) {
        rest.split('"').next().unwrap_or("")
    } else if let Some(rest) = line.split("inet_pton(AF_INET6, \"").nth(1) {
        rest.split('"').next().unwrap_or("")
    } else {
        return;
    };
    let loopback = addr.starts_with("127.") || addr == "::1" || addr == "::ffff:127.0.0.1";
    assert!(
        loopback,
        "{}: connection that bypassed the proxy: {line}",
        trace.display()
    );
}

/// Syscalls that create, change or remove files.
fn is_write(line: &str) -> bool {
    let call = line
        .split_once(' ')
        .map_or(line, |(_, rest)| rest)
        .split('(')
        .next()
        .unwrap_or("");
    match call {
        "open" | "openat" | "openat2" | "creat" => {
            ["O_WRONLY", "O_RDWR", "O_CREAT", "O_TRUNC"]
                .iter()
                .any(|f| line.contains(f))
                || call == "creat"
        }
        "mkdir" | "mkdirat" | "rename" | "renameat" | "renameat2" | "unlink" | "unlinkat"
        | "rmdir" | "link" | "linkat" | "symlink" | "symlinkat" | "truncate" | "chmod"
        | "fchmodat" => true,
        _ => false,
    }
}

/// The `"..."` arguments of an strace line.
fn quoted(line: &str) -> Vec<String> {
    line.split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_owned())
        .collect()
}
