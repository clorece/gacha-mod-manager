// SPDX-License-Identifier: MIT OR Apache-2.0

//! RAR through the real helper: benign archives extract exactly, and the malicious corpus
//! (docs/04-security.md, archive hardening) is rejected with nothing left behind and nothing
//! written outside the root (checked in `Env::extract`).

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

mod common;

use common::{Env, R5, rar4, rar5};
use gmm_archive::{ArchiveError, Format, Helper, Limits, extract_with};
use gmm_fsguard::PathError;

fn small_limits() -> Limits {
    Limits {
        max_total_bytes: 1024 * 1024,
        max_entries: 10,
        max_ratio: 100,
        ratio_floor_bytes: 64 * 1024,
        max_path_len: 60,
        max_depth: 4,
    }
}

// ---- benign ---------------------------------------------------------------------------------

#[test]
fn benign_rar5_extracts() {
    let env = Env::new();
    let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let bytes = rar5(
        &[
            R5::dir("Velina"),
            R5::file(
                "Velina/Velina.ini",
                b"[TextureOverrideBody]\nhash = 1234abcd\n",
            ),
            R5::file("Velina/Textures/body.dds", &big),
            R5::file("readme.txt", b"thanks for downloading"),
        ],
        false,
    );
    let report = env.extract(&bytes, &Limits::default()).unwrap();
    assert_eq!(report.format, Format::Rar);
    let files: Vec<String> = report.files().map(|e| e.path.to_string()).collect();
    assert_eq!(
        files,
        [
            "Velina/Textures/body.dds",
            "Velina/Velina.ini",
            "readme.txt"
        ]
    );
    assert_eq!(env.read(&report, "Velina/Textures/body.dds"), big);
    assert_eq!(
        env.read(&report, "Velina/Velina.ini"),
        b"[TextureOverrideBody]\nhash = 1234abcd\n"
    );
    assert_eq!(report.total_bytes, big.len() as u64 + 38 + 22);
}

#[test]
fn benign_rar4_extracts() {
    let env = Env::new();
    let bytes = rar4(&[
        ("Hsin/", b""),
        ("Hsin/Hsin.ini", b"[Constants]\n"),
        ("Hsin/tex.dds", &[7u8; 5000]),
    ]);
    let report = env.extract(&bytes, &Limits::default()).unwrap();
    let files: Vec<String> = report.files().map(|e| e.path.to_string()).collect();
    assert_eq!(files, ["Hsin/Hsin.ini", "Hsin/tex.dds"]);
    assert_eq!(env.read(&report, "Hsin/tex.dds"), vec![7u8; 5000]);
}

#[test]
fn empty_files_and_unicode_names() {
    let env = Env::new();
    let bytes = rar5(
        &[
            R5::file("Ellen/empty.txt", b""),
            R5::file("Ellen/エレン.ini", b"x"),
        ],
        false,
    );
    let report = env.extract(&bytes, &Limits::default()).unwrap();
    assert_eq!(env.read(&report, "Ellen/empty.txt"), b"");
    assert_eq!(env.read(&report, "Ellen/エレン.ini"), b"x");
}

// ---- unsafe names ---------------------------------------------------------------------------

#[test]
fn rar5_path_traversal_rejected() {
    let env = Env::new();
    for name in [
        "../evil.ini",
        "Mod/../../evil.ini",
        "/abs/evil.ini",
        "C:/Windows/evil.ini",
        "Mod/CON.ini",
        "Mod/a.ini:stream",
    ] {
        let bytes = rar5(&[R5::file("ok.txt", b"ok"), R5::file(name, b"bad")], false);
        // GMM rejects the name, or UnRAR has already rewritten it into a harmless relative
        // one (it turns `C:` into `C_`); either way nothing lands outside the destination,
        // which `Env::extract` checks, and every written path passed GMM's own checks.
        match env.extract(&bytes, &Limits::default()) {
            Err(ArchiveError::UnsafePath { .. }) => {}
            Ok(report) => {
                for e in &report.entries {
                    let p = e.path.to_string();
                    assert!(
                        !p.contains("..") && !p.contains(':'),
                        "{name:?} became {p:?}"
                    );
                }
                assert_ne!(name, "../evil.ini", "a leading .. must never pass");
            }
            other => panic!("{name:?} gave {other:?}"),
        }
    }
}

#[test]
fn rar4_backslash_traversal_rejected() {
    let env = Env::new();
    let bytes = rar4(&[("ok.txt", b"ok"), ("..\\..\\evil.ini", b"bad")]);
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::UnsafePath { .. })
    ));
}

#[test]
fn rar5_dot_dot_reason_is_reported() {
    let env = Env::new();
    let bytes = rar5(&[R5::file("../x", b"")], false);
    match env.extract(&bytes, &Limits::default()) {
        Err(ArchiveError::UnsafePath { reason, .. }) => assert_eq!(reason, PathError::ParentDir),
        other => panic!("unexpected {other:?}"),
    }
}

// ---- links and special files ----------------------------------------------------------------

#[test]
fn rar5_symlink_rejected() {
    let env = Env::new();
    let bytes = rar5(
        &[
            R5::file("a.ini", b"x"),
            R5::symlink("link", "../../outside"),
        ],
        false,
    );
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::Symlink(name)) if name == "link"
    ));
}

#[test]
fn rar5_hardlink_rejected() {
    let env = Env::new();
    let bytes = rar5(
        &[R5::file("a.ini", b"x"), R5::hardlink("b.ini", "a.ini")],
        false,
    );
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::Symlink(_))
    ));
}

#[test]
fn rar5_unix_special_files_rejected() {
    let env = Env::new();
    // FIFO, then a symlink given only by its Unix mode.
    for mode in [0o010_644, 0o120_777] {
        let bytes = rar5(&[R5::unix("pipe", mode)], false);
        let result = env.extract(&bytes, &Limits::default());
        assert!(
            matches!(
                result,
                Err(ArchiveError::SpecialFile(_) | ArchiveError::Symlink(_))
            ),
            "mode {mode:o} gave {result:?}"
        );
    }
}

// ---- collisions and limits ------------------------------------------------------------------

#[test]
fn rar5_case_collision_rejected() {
    let env = Env::new();
    let bytes = rar5(
        &[R5::file("Mod/a.ini", b"1"), R5::file("MOD/A.INI", b"2")],
        false,
    );
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::Collision(_))
    ));
}

#[test]
fn rar5_too_many_entries_rejected() {
    let env = Env::new();
    let names: Vec<String> = (0..11).map(|i| format!("f{i}.txt")).collect();
    let entries: Vec<R5> = names.iter().map(|n| R5::file(n, b"x")).collect();
    assert!(matches!(
        env.extract(&rar5(&entries, false), &small_limits()),
        Err(ArchiveError::TooManyEntries { limit: 10 })
    ));
}

#[test]
fn rar5_too_deep_and_too_long_rejected() {
    let env = Env::new();
    let deep = rar5(&[R5::file("a/b/c/d/e/f.txt", b"x")], false);
    assert!(matches!(
        env.extract(&deep, &small_limits()),
        Err(ArchiveError::TooDeep { .. })
    ));
    let long = rar5(&[R5::file(&"n".repeat(61), b"x")], false);
    assert!(matches!(
        env.extract(&long, &small_limits()),
        Err(ArchiveError::PathTooLong { .. })
    ));
}

#[test]
fn rar5_declared_size_over_limit_rejected_before_writing() {
    let env = Env::new();
    let mut lying = R5::file("huge.dds", b"tiny");
    lying.declared = Some(10 * 1024 * 1024);
    let bytes = rar5(&[R5::file("first.txt", b"x"), lying], false);
    assert!(matches!(
        env.extract(&bytes, &small_limits()),
        Err(ArchiveError::TooLarge { .. })
    ));
}

#[test]
fn rar5_actual_bytes_counted_even_when_sizes_lie() {
    // Each file claims to be tiny; the real data adds up past the limit.
    let env = Env::new();
    let chunk = vec![1u8; 400 * 1024];
    let entries: Vec<R5> = (0..3)
        .map(|i| {
            let mut e = R5::file(&format!("f{i}.dds"), &chunk);
            e.declared = Some(1);
            e
        })
        .collect();
    let result = env.extract(&rar5(&entries, false), &small_limits());
    assert!(
        matches!(
            result,
            Err(ArchiveError::TooLarge { .. } | ArchiveError::Corrupt(_))
        ),
        "{result:?}"
    );
}

// ---- encryption -----------------------------------------------------------------------------

#[test]
fn rar5_encrypted_headers_rejected() {
    let env = Env::new();
    let bytes = rar5(&[R5::file("a.ini", b"x")], true);
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::Encrypted)
    ));
}

#[test]
fn rar5_encrypted_file_rejected() {
    let env = Env::new();
    let bytes = rar5(
        &[R5::file("a.ini", b"x"), R5::encrypted("b.dds", &[0u8; 64])],
        false,
    );
    assert!(matches!(
        env.extract(&bytes, &Limits::default()),
        Err(ArchiveError::Encrypted)
    ));
}

// ---- damaged input and the helper itself ----------------------------------------------------

#[test]
fn truncated_and_garbage_rar_fail_cleanly() {
    let env = Env::new();
    let good = rar5(
        &[R5::file("a.dds", &[5u8; 20_000]), R5::file("b.ini", b"x")],
        false,
    );
    // A cut either fails or, when it only lost the end (or everything after the signature),
    // yields the complete files and nothing partial.
    for cut in [12, 40, good.len() / 2, good.len() - 20, good.len() - 4] {
        if let Ok(report) = env.extract(&good[..cut], &Limits::default()) {
            for f in report.files() {
                let want: &[u8] = if f.path.to_string() == "a.dds" {
                    &[5u8; 20_000]
                } else {
                    b"x"
                };
                assert_eq!(env.read(&report, &f.path.to_string()), want, "cut at {cut}");
            }
        }
    }
    assert!(
        env.extract(&good[..good.len() / 2], &Limits::default())
            .is_err()
    );
    let mut garbage = common::RAR5_SIGNATURE.to_vec();
    garbage.extend_from_slice(&[0xFF; 64]);
    assert!(env.extract(&garbage, &Limits::default()).is_err());
    // A flipped byte in the first file header breaks its header CRC; one in the data breaks
    // the file's CRC, which UnRAR checks while testing.
    let mut flipped = good.clone();
    flipped[common::RAR5_SIGNATURE.len() + 8 + 6] ^= 0x55;
    assert!(env.extract(&flipped, &Limits::default()).is_err());
    let mut flipped = good.clone();
    let in_data = flipped.len() / 3;
    flipped[in_data] ^= 0x55;
    assert!(matches!(
        env.extract(&flipped, &Limits::default()),
        Err(ArchiveError::Corrupt(_))
    ));
}

#[test]
fn missing_helper_is_reported() {
    let env = Env::new();
    let (archive, dest) = env.stage(&rar5(&[R5::file("a", b"x")], false));
    let missing = Helper::at(env.outside.join("gone").join("gmm-extract.exe"));
    assert!(matches!(
        extract_with(
            &env.guard,
            &archive,
            &dest,
            &Limits::default(),
            Some(&missing)
        ),
        Err(ArchiveError::HelperMissing(_))
    ));
    assert!(!env.guard.exists(&dest).unwrap());
}

#[test]
fn helper_refuses_to_run_by_hand() {
    // Fixed arguments only: anything but exactly "list" or "extract" is refused.
    #[allow(clippy::disallowed_methods)]
    for args in [&[][..], &["unpack"][..], &["list", "extra"][..]] {
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_gmm-extract"))
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(2), "{args:?}");
    }
}
