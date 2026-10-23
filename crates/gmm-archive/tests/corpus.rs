// SPDX-License-Identifier: MIT OR Apache-2.0

//! Malicious archive corpus (docs/04-security.md, Phase 1 harness). Every case must be
//! rejected with the expected error, leave no destination behind, and write nothing outside
//! the root (checked inside `Env::extract`).

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

mod common;

use common::{E, Env, mark_zip_encrypted, patch, sevenz_bytes, zip_bytes, zip_stored};
use gmm_archive::{ArchiveError, Format, Limits};
use gmm_fsguard::PathError;
use zip::CompressionMethod;

fn small_limits() -> Limits {
    Limits {
        max_total_bytes: 64 * 1024 * 1024,
        max_entries: 10,
        max_ratio: 100,
        ratio_floor_bytes: 1024 * 1024,
        max_path_len: 60,
        max_depth: 4,
    }
}

// ---- benign archives ------------------------------------------------------------------------

#[test]
fn benign_zip_extracts() {
    let env = Env::new();
    let ini: &[u8] = b"[TextureOverrideEllenBody]\nhash = 1a2b3c4d\n";
    let bytes = zip_bytes(
        &[
            E::Dir("Ellen Maid/"),
            E::File("Ellen Maid/Ellen.ini", ini),
            E::File("Ellen Maid/Textures/Body.dds", &[7u8; 4096]),
            E::File(r"Ellen Maid\Buffers\Body.ib", &[1u8; 128]),
        ],
        CompressionMethod::Deflated,
    );
    let report = env.extract(&bytes, &Limits::default()).unwrap();
    assert_eq!(report.format, Format::Zip);
    // Folders created only to hold a file (Buffers/, Textures/) are not separate entries.
    let paths: Vec<_> = report.entries.iter().map(|e| e.path.to_string()).collect();
    assert_eq!(
        paths,
        [
            "Ellen Maid",
            "Ellen Maid/Buffers/Body.ib",
            "Ellen Maid/Ellen.ini",
            "Ellen Maid/Textures/Body.dds",
        ]
    );
    let tex = report.dest.join("Ellen Maid/Textures/Body.dds").unwrap();
    assert_eq!(env.guard.read(&tex).unwrap().len(), 4096);
    assert_eq!(report.total_bytes, 4096 + 128 + ini.len() as u64);
    let ini = report.dest.join("Ellen Maid/Ellen.ini").unwrap();
    assert!(
        env.guard
            .read_to_string(&ini)
            .unwrap()
            .contains("hash = 1a2b3c4d")
    );
}

#[test]
fn benign_7z_extracts() {
    let env = Env::new();
    let ini: &[u8] = b"[ShaderOverrideA]\nhash = 0011\n";
    let bytes = sevenz_bytes(&[
        E::Dir("Mod"),
        E::File("Mod/mod.ini", ini),
        E::File("Mod/empty.txt", b""),
        E::File("Mod/tex/a.dds", &[3u8; 10_000]),
    ]);
    let report = env.extract(&bytes, &Limits::default()).unwrap();
    assert_eq!(report.format, Format::SevenZ);
    assert_eq!(report.files().count(), 3);
    assert_eq!(report.total_bytes, 10_000 + ini.len() as u64);
    let dds = report.dest.join("Mod/tex/a.dds").unwrap();
    assert_eq!(env.guard.read(&dds).unwrap(), vec![3u8; 10_000]);
}

// ---- path traversal and unsafe names --------------------------------------------------------

fn assert_unsafe(result: Result<gmm_archive::ExtractReport, ArchiveError>, want: PathError) {
    match result {
        Err(ArchiveError::UnsafePath { reason, .. }) => assert_eq!(
            std::mem::discriminant(&reason),
            std::mem::discriminant(&want),
            "got {reason:?}"
        ),
        other => panic!("expected UnsafePath({want:?}), got {other:?}"),
    }
}

/// Builds a zip whose single file is named `name` by patching a same-length placeholder.
fn zip_named(name: &str) -> Vec<u8> {
    let placeholder: String = "Q".repeat(name.len());
    patch(
        zip_stored(&[E::File(&placeholder, b"pwned")]),
        &placeholder,
        name,
    )
}

#[test]
fn zip_slip_variants_rejected() {
    let env = Env::new();
    let l = Limits::default();
    assert_unsafe(
        env.extract(&zip_named("../evil.txt"), &l),
        PathError::ParentDir,
    );
    assert_unsafe(
        env.extract(&zip_named("a/../../evil.txt"), &l),
        PathError::ParentDir,
    );
    assert_unsafe(
        env.extract(&zip_named(r"..\..\evil.txt"), &l),
        PathError::ParentDir,
    );
    assert_unsafe(
        env.extract(&zip_named("/etc/evil.txt"), &l),
        PathError::Absolute,
    );
    assert_unsafe(
        env.extract(&zip_named(r"\\srv\share\x"), &l),
        PathError::Absolute,
    );
    assert_unsafe(
        env.extract(&zip_named("C:/Windows/evil"), &l),
        PathError::Colon(String::new()),
    );
    assert_unsafe(
        env.extract(&zip_named(r"C:\evil.txt"), &l),
        PathError::Colon(String::new()),
    );
}

#[test]
fn windows_name_tricks_rejected() {
    let env = Env::new();
    let l = Limits::default();
    // Alternate data stream: would hide data behind an innocent-looking file.
    assert_unsafe(
        env.extract(&zip_named("mod.ini:hidden"), &l),
        PathError::Colon(String::new()),
    );
    assert_unsafe(
        env.extract(&zip_named("CON.ini"), &l),
        PathError::Reserved(String::new()),
    );
    assert_unsafe(
        env.extract(&zip_named("aux/x.ini"), &l),
        PathError::Reserved(String::new()),
    );
    assert_unsafe(
        env.extract(&zip_named("x.ini."), &l),
        PathError::TrailingDotOrSpace(String::new()),
    );
    assert_unsafe(
        env.extract(&zip_named("dir /x.ini"), &l),
        PathError::TrailingDotOrSpace(String::new()),
    );
    assert_unsafe(
        env.extract(&zip_named("a?b.ini"), &l),
        PathError::InvalidChar(String::new()),
    );
}

#[test]
fn sevenz_traversal_rejected() {
    let env = Env::new();
    let bytes = sevenz_bytes(&[E::File("ok.ini", b"x"), E::File("../evil.txt", b"pwned")]);
    assert_unsafe(
        env.extract(&bytes, &Limits::default()),
        PathError::ParentDir,
    );
}

// ---- links and special files ----------------------------------------------------------------

#[test]
fn zip_symlink_rejected() {
    let env = Env::new();
    let bytes = zip_stored(&[E::File("ok.ini", b"x"), E::Link("evil", "../../../outside")]);
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::Symlink(_))
    ));
}

#[test]
fn sevenz_symlink_rejected() {
    let env = Env::new();
    let bytes = sevenz_bytes(&[E::File("ok.ini", b"x"), E::Link("evil", "/etc")]);
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::Symlink(_))
    ));
}

// ---- collisions -----------------------------------------------------------------------------

#[test]
fn case_collision_rejected() {
    let env = Env::new();
    let zip = zip_stored(&[E::File("Mod/a.ini", b"1"), E::File("MOD/A.INI", b"2")]);
    assert!(matches!(
        env.extract(&zip, &Limits::default()),
        Err(ArchiveError::Collision(_))
    ));
    let sz = sevenz_bytes(&[E::File("Mod/a.ini", b"1"), E::File("mod/A.ini", b"2")]);
    assert!(matches!(
        env.extract(&sz, &Limits::default()),
        Err(ArchiveError::Collision(_))
    ));
}

#[test]
fn file_versus_folder_collision_rejected() {
    let env = Env::new();
    let zip = zip_stored(&[E::File("a", b"file"), E::File("a/b.ini", b"x")]);
    assert!(matches!(
        env.extract(&zip, &Limits::default()),
        Err(ArchiveError::Collision(_))
    ));
}

#[test]
fn duplicate_folder_entries_are_fine() {
    let env = Env::new();
    let zip = zip_stored(&[E::Dir("Mod/"), E::File("Mod/a.ini", b"1")]);
    let zip = patch(zip, "Mod/a.ini", "Mod/a.ini"); // no-op, keeps the helper honest
    env.extract(&zip, &Limits::default()).unwrap();
}

// ---- resource limits ------------------------------------------------------------------------

#[test]
fn too_many_entries_rejected() {
    let env = Env::new();
    let names: Vec<String> = (0..11).map(|i| format!("f{i}.ini")).collect();
    let entries: Vec<E> = names.iter().map(|n| E::File(n, b"x")).collect();
    assert!(matches!(
        env.extract(&zip_stored(&entries), &small_limits()),
        Err(ArchiveError::TooManyEntries { limit: 10 })
    ));
}

#[test]
fn too_deep_rejected() {
    let env = Env::new();
    let zip = zip_stored(&[E::File("a/b/c/d/e.ini", b"x")]);
    assert!(matches!(
        env.extract(&zip, &small_limits()),
        Err(ArchiveError::TooDeep { limit: 4, .. })
    ));
}

#[test]
fn path_too_long_rejected() {
    let env = Env::new();
    let name = format!("{}.ini", "x".repeat(70));
    let zip = zip_stored(&[E::File(&name, b"x")]);
    assert!(matches!(
        env.extract(&zip, &small_limits()),
        Err(ArchiveError::PathTooLong { limit: 60, .. })
    ));
}

#[test]
fn declared_size_over_limit_rejected_before_writing() {
    let env = Env::new();
    let limits = Limits {
        max_total_bytes: 1024 * 1024,
        ..small_limits()
    };
    let zip = zip_stored(&[E::File("big.dds", &vec![0u8; 2 * 1024 * 1024])]);
    assert!(matches!(
        env.extract(&zip, &limits),
        Err(ArchiveError::TooLarge { .. })
    ));
}

#[test]
fn zip_bomb_rejected_per_entry() {
    let env = Env::new();
    let zeros = vec![0u8; 8 * 1024 * 1024];
    let zip = zip_bytes(&[E::File("bomb.dds", &zeros)], CompressionMethod::Deflated);
    assert!(zip.len() < 64 * 1024, "test archive should compress well");
    assert!(matches!(
        env.extract(&zip, &small_limits()),
        Err(ArchiveError::Bomb { .. })
    ));
}

#[test]
fn sevenz_bomb_rejected_by_archive_ratio() {
    // 7z has no per-entry compressed size, so this exercises the whole-archive ratio check.
    let env = Env::new();
    let zeros = vec![0u8; 8 * 1024 * 1024];
    let bytes = sevenz_bytes(&[E::File("bomb.dds", &zeros)]);
    assert!(matches!(
        env.extract(&bytes, &small_limits()),
        Err(ArchiveError::Bomb { .. })
    ));
}

// ---- other rejections -----------------------------------------------------------------------

#[test]
fn encrypted_zip_rejected() {
    let env = Env::new();
    let zip = mark_zip_encrypted(zip_stored(&[E::File("mod.ini", b"x")]));
    assert!(matches!(
        env.extract(&zip, &Limits::default()),
        Err(ArchiveError::Encrypted)
    ));
}

#[test]
fn unknown_and_unsupported_formats() {
    let env = Env::new();
    assert!(matches!(
        env.extract(b"just some text, not an archive", &Limits::default()),
        Err(ArchiveError::UnknownFormat)
    ));
    assert!(matches!(
        env.extract(b"MZ\x90\x00\x03\x00\x00\x00", &Limits::default()),
        Err(ArchiveError::UnknownFormat)
    ));
    assert!(matches!(
        env.extract(b"Rar!\x1A\x07\x01\x00rest", &Limits::default()),
        Err(ArchiveError::UnsupportedFormat("RAR"))
    ));
    assert!(matches!(
        env.extract(b"", &Limits::default()),
        Err(ArchiveError::UnknownFormat)
    ));
}

#[test]
fn truncated_archives_fail_cleanly() {
    let env = Env::new();
    let zip = zip_bytes(
        &[E::File("a.dds", &[9u8; 50_000])],
        CompressionMethod::Deflated,
    );
    let sz = sevenz_bytes(&[E::File("a.dds", &[9u8; 50_000])]);
    for bytes in [&zip, &sz] {
        for cut in [bytes.len() / 3, bytes.len() / 2, bytes.len() - 10] {
            assert!(env.extract(&bytes[..cut], &Limits::default()).is_err());
        }
    }
}

#[test]
fn existing_destination_refused() {
    let env = Env::new();
    let (archive, dest) = env.stage(&zip_stored(&[E::File("a.ini", b"x")]));
    env.guard.create_dir_all(&dest).unwrap();
    assert!(matches!(
        gmm_archive::extract(&env.guard, &archive, &dest, &Limits::default()),
        Err(ArchiveError::DestinationExists)
    ));
    // The existing destination is the caller's; it must not be removed.
    assert!(env.guard.exists(&dest).unwrap());
}

#[test]
fn rejected_late_entry_leaves_nothing_behind() {
    // The unsafe entry comes after a valid one; validation runs before any write, so not
    // even the valid file is created (Env::extract also checks dest is gone).
    let env = Env::new();
    let zip = patch(
        zip_stored(&[
            E::File("good.ini", b"fine"),
            E::File("QQQQQQQQQQQ", b"pwned"),
        ]),
        "QQQQQQQQQQQ",
        "../evil.ini",
    );
    assert!(env.extract(&zip, &Limits::default()).is_err());
}
