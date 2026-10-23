// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

mod common;

use common::{E, Env, zip_stored};
use gmm_archive::{FindingKind, Limits, ScanOptions, Severity, scan};

fn findings_for(entries: &[E]) -> Vec<(String, Severity, FindingKind)> {
    let env = Env::new();
    let report = env
        .extract(&zip_stored(entries), &Limits::default())
        .unwrap();
    scan(&env.guard, &report, &ScanOptions::default())
        .unwrap()
        .findings
        .into_iter()
        .map(|f| (f.path.to_string(), f.severity, f.kind))
        .collect()
}

#[test]
fn clean_mod_has_no_findings() {
    let f = findings_for(&[
        E::File(
            "Mod/mod.ini",
            b"[TextureOverrideBody]\nhash = 1234abcd\nps-t0 = Resource\n",
        ),
        E::File("Mod/Body.dds", b"DDS \x7c\x00\x00\x00rest"),
        E::File("Mod/Body.ib", &[0u8; 64]),
        E::File("Mod/preview.png", b"\x89PNG\r\n\x1a\n"),
    ]);
    assert!(f.is_empty(), "{f:?}");
}

#[test]
fn executables_blocked_by_extension_and_content() {
    let f = findings_for(&[
        E::File("Mod/Installer.EXE", b"MZ\x90\x00"),
        E::File("Mod/run.bat", b"@echo off"),
        E::File("Mod/textures/skin.dds", b"MZ\x90\x00disguised"),
        E::File("Mod/setup.sh", b"#!/bin/sh\nrm -rf ~"),
    ]);
    assert_eq!(
        f,
        [
            (
                "Mod/Installer.EXE".into(),
                Severity::Blocked,
                FindingKind::ExecutableExtension("exe".into())
            ),
            (
                "Mod/run.bat".into(),
                Severity::Blocked,
                FindingKind::ExecutableExtension("bat".into())
            ),
            (
                "Mod/setup.sh".into(),
                Severity::Blocked,
                FindingKind::ExecutableExtension("sh".into())
            ),
            (
                "Mod/textures/skin.dds".into(),
                Severity::Blocked,
                FindingKind::ExecutableContent
            ),
        ]
    );
}

#[test]
fn warnings_for_loader_files_ini_escapes_and_nested_archives() {
    let f = findings_for(&[
        E::File(
            "Mod/d3dx.ini",
            b"[Loader]\ntarget = Client-Win64-Shipping.exe\n",
        ),
        E::File(
            "Mod/mod.ini",
            b"[ResourceBody]\nfilename = ..\\..\\Other\\Body.dds\n",
        ),
        E::File("Mod/extra.zip", b"PK\x03\x04rest"),
    ]);
    assert_eq!(
        f,
        [
            (
                "Mod/d3dx.ini".into(),
                Severity::Warning,
                FindingKind::LoaderFile
            ),
            (
                "Mod/extra.zip".into(),
                Severity::Warning,
                FindingKind::NestedArchive
            ),
            (
                "Mod/mod.ini".into(),
                Severity::Warning,
                FindingKind::IniEscapesFolder
            ),
        ]
    );
}

#[test]
fn report_helpers() {
    let env = Env::new();
    let report = env
        .extract(
            &zip_stored(&[E::File("a.exe", b"MZ"), E::File("d3dx.ini", b"")]),
            &Limits::default(),
        )
        .unwrap();
    let scan = scan(&env.guard, &report, &ScanOptions::default()).unwrap();
    assert!(scan.is_blocked());
    assert_eq!(scan.blocked().count(), 1);
    assert_eq!(scan.warnings().count(), 1);
}
