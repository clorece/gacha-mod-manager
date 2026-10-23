// SPDX-License-Identifier: MIT OR Apache-2.0

// Tests set up fixtures with std::fs directly; the code under test still goes through the guard.
// Fixture helpers aren't `#[test]` functions, so clippy's in-test unwrap allowance misses them.
#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::fs;
use std::path::Path;

use gmm_fsguard::{Access, FsError, Guard, PathError};
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    guard: Guard,
    rw: gmm_fsguard::RootId,
    ro: gmm_fsguard::RootId,
    outside: std::path::PathBuf,
}

/// Layout:
///   <tmp>/mods      read-write root
///   <tmp>/game      read-only root, contains game.exe
///   <tmp>/outside   not a root, contains secret.txt
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path().join("mods");
    let game = dir.path().join("game");
    let outside = dir.path().join("outside");
    for d in [&mods, &game, &outside] {
        fs::create_dir(d).unwrap();
    }
    fs::write(game.join("game.exe"), b"exe").unwrap();
    fs::write(outside.join("secret.txt"), b"secret").unwrap();

    let mut guard = Guard::new();
    let rw = guard
        .register_root("mods", &mods, Access::ReadWrite)
        .unwrap();
    let ro = guard
        .register_root("game", &game, Access::ReadOnly)
        .unwrap();
    Fixture {
        _dir: dir,
        guard,
        rw,
        ro,
        outside,
    }
}

#[test]
fn read_write_inside_rw_root() {
    let f = fixture();
    let dir = f.guard.path(f.rw, "GMM/Ellen [gb-1]").unwrap();
    f.guard.create_dir_all(&dir).unwrap();
    let ini = dir.join("Ellen.ini").unwrap();
    f.guard.write_atomic(&ini, b"[TextureOverrideA]").unwrap();
    assert_eq!(f.guard.read(&ini).unwrap(), b"[TextureOverrideA]");

    // Replacing an existing file works and leaves no temp files behind.
    f.guard.write_atomic(&ini, b"v2").unwrap();
    assert_eq!(f.guard.read_to_string(&ini).unwrap(), "v2");
    let names: Vec<_> = f
        .guard
        .read_dir(&dir)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["Ellen.ini"]);
}

#[test]
fn append_and_ensure_root() {
    let f = fixture();
    let part = f.guard.path(f.rw, "dl.part").unwrap();
    f.guard.write_atomic(&part, b"abc").unwrap();
    {
        use std::io::Write;
        let mut file = f.guard.open_append(&part).unwrap();
        file.write_all(b"def").unwrap();
    }
    assert_eq!(f.guard.read(&part).unwrap(), b"abcdef");
    let exe = f.guard.path(f.ro, "game.exe").unwrap();
    assert!(matches!(
        f.guard.open_append(&exe),
        Err(FsError::ReadOnlyRoot(_))
    ));

    let dir = tempfile::tempdir().unwrap();
    let mut guard = Guard::new();
    let app = dir.path().join("GMM").join("cache");
    let id = guard.ensure_root("cache", &app, Access::ReadWrite).unwrap();
    assert!(app.is_dir());
    assert_eq!(guard.root_access(id), Some(Access::ReadWrite));
    assert!(matches!(
        guard.ensure_root("bad", &dir.path().join(".ssh"), Access::ReadWrite),
        Err(FsError::Denied(_))
    ));
    assert!(!dir.path().join(".ssh").exists());
}

#[test]
fn rename_toggles_within_root() {
    let f = fixture();
    let enabled = f.guard.path(f.rw, "GMM/Ellen").unwrap();
    let disabled = f.guard.path(f.rw, "GMM/DISABLED Ellen").unwrap();
    f.guard.create_dir_all(&enabled).unwrap();
    f.guard.rename(&enabled, &disabled).unwrap();
    assert!(!f.guard.exists(&enabled).unwrap());
    assert!(f.guard.exists(&disabled).unwrap());
}

#[test]
fn read_only_root_refuses_writes() {
    let f = fixture();
    let exe = f.guard.path(f.ro, "game.exe").unwrap();
    assert_eq!(f.guard.read(&exe).unwrap(), b"exe");
    assert!(matches!(
        f.guard.write_atomic(&exe, b"x"),
        Err(FsError::ReadOnlyRoot(_))
    ));
    assert!(matches!(
        f.guard.remove_file(&exe),
        Err(FsError::ReadOnlyRoot(_))
    ));
    let new_dir = f.guard.path(f.ro, "Mods").unwrap();
    assert!(matches!(
        f.guard.create_dir_all(&new_dir),
        Err(FsError::ReadOnlyRoot(_))
    ));
}

#[test]
fn relative_escapes_rejected_at_construction() {
    let f = fixture();
    assert!(matches!(
        f.guard.path(f.rw, "../outside/secret.txt"),
        Err(FsError::InvalidPath(PathError::ParentDir))
    ));
    assert!(matches!(
        f.guard.path(f.rw, "C:/Windows/win.ini"),
        Err(FsError::InvalidPath(PathError::Colon(_)))
    ));
}

#[test]
fn cross_root_rename_rejected() {
    let f = fixture();
    let a = f.guard.path(f.rw, "a").unwrap();
    let b = f.guard.path(f.ro, "b").unwrap();
    assert!(matches!(f.guard.rename(&a, &b), Err(FsError::CrossRoot)));
}

#[test]
fn root_itself_cannot_be_removed() {
    let f = fixture();
    let root = f.guard.path(f.rw, "").unwrap();
    assert!(f.guard.remove_dir_all(&root).is_err());
    assert!(f.guard.exists(&root).unwrap());
}

#[test]
fn denied_names_blocked_even_inside_roots() {
    let f = fixture();
    let creds = f.guard.path(f.rw, "config/loginusers.vdf").unwrap();
    assert!(matches!(
        f.guard.write_atomic(&creds, b"x"),
        Err(FsError::Denied(_))
    ));
    assert!(matches!(f.guard.read(&creds), Err(FsError::Denied(_))));

    // Denied entries created by someone else are hidden from listings.
    let root = f.guard.path(f.rw, "").unwrap();
    let real = f.guard.root_path(f.rw).unwrap().to_path_buf();
    fs::write(real.join("Player.log"), b"uid=123").unwrap();
    fs::write(real.join("ok.ini"), b"").unwrap();
    let names: Vec<_> = f
        .guard
        .read_dir(&root)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["ok.ini"]);
}

#[test]
fn denied_root_cannot_be_registered() {
    let dir = tempfile::tempdir().unwrap();
    let userdata = dir.path().join("userdata");
    fs::create_dir(&userdata).unwrap();
    let mut guard = Guard::new();
    assert!(matches!(
        guard.register_root("steam userdata", &userdata, Access::ReadOnly),
        Err(FsError::Denied(_))
    ));
}

#[test]
fn register_root_validation() {
    let mut guard = Guard::new();
    assert!(matches!(
        guard.register_root("rel", Path::new("relative/dir"), Access::ReadOnly),
        Err(FsError::RootNotAbsolute(_))
    ));
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    assert!(matches!(
        guard.register_root("missing", &missing, Access::ReadOnly),
        Err(FsError::RootNotADirectory(_))
    ));
    let a = guard
        .register_root("a", dir.path(), Access::ReadOnly)
        .unwrap();
    let b = guard
        .register_root("b", dir.path(), Access::ReadOnly)
        .unwrap();
    assert_eq!(a, b, "same dir + access is deduplicated");
}

#[test]
fn scope_absolute_maps_to_most_specific_root() {
    let f = fixture();
    let real_mods = f.guard.root_path(f.rw).unwrap().to_path_buf();
    let p = f
        .guard
        .scope_absolute(&real_mods.join("GMM").join("new.ini"))
        .unwrap();
    assert_eq!(p.root(), f.rw);
    assert_eq!(p.rel().to_string(), "GMM/new.ini");

    assert!(matches!(
        f.guard.scope_absolute(&f.outside.join("secret.txt")),
        Err(FsError::OutsideRoots(_))
    ));
    // `..` in a not-yet-existing tail must not sneak out either.
    let sneaky = real_mods.join("nope").join("..").join("..").join("outside");
    assert!(f.guard.scope_absolute(&sneaky).is_err());
}

#[test]
fn paths_from_another_guard_rejected() {
    let f = fixture();
    let other = Guard::new();
    let p = f.guard.path(f.rw, "x").unwrap();
    assert!(matches!(other.read(&p), Err(FsError::ForeignPath)));
}

/// A junction (Windows) or symlink (Unix) inside a root that points outside it must not be
/// followed, for reads or writes.
#[test]
fn link_escape_blocked() {
    let f = fixture();
    let real_mods = f.guard.root_path(f.rw).unwrap().to_path_buf();
    let link = real_mods.join("evil");
    if !make_dir_link(&f.outside, &link) {
        eprintln!("skipping: could not create a directory link on this system");
        return;
    }
    let secret = f.guard.path(f.rw, "evil/secret.txt").unwrap();
    assert!(matches!(f.guard.read(&secret), Err(FsError::Escape(_))));
    let planted = f.guard.path(f.rw, "evil/planted.txt").unwrap();
    assert!(matches!(
        f.guard.write_atomic(&planted, b"x"),
        Err(FsError::Escape(_))
    ));
    assert!(!f.outside.join("planted.txt").exists());
}

#[cfg(windows)]
fn make_dir_link(target: &Path, link: &Path) -> bool {
    // Junctions need no special privilege, unlike symlinks.
    std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn make_dir_link(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}
