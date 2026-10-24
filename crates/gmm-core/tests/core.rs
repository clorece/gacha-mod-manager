// SPDX-License-Identifier: MIT OR Apache-2.0

//! gmm-core on a fake XXMI importer folder in a temp dir: install pipeline, library, conflicts.
//! Archives are built in code; GameBanana is a mock server for the end-to-end tests.

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::{GameBanana, NoCache};
use gmm_core::{
    Analysis, CoreError, InstallOptions, InstallRequest, ModMeta, Sidecar, VariantChoice,
    Workspace, analyze, fetch_archive, install, install_analyzed,
};
use gmm_db::Db;
use gmm_fsguard::{RootId, ScopedPath};
use gmm_net::{Allowlist, Client, NetConfig, RateLimit, Url};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zip::write::{SimpleFileOptions, ZipWriter};

const INI: &[u8] = b"[TextureOverrideBody]\nhash = 0123abcd\nib = ResourceBodyIB\n";

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        w.start_file(*name, SimpleFileOptions::default()).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn zzz() -> &'static GameManifest {
    Registry::builtin().unwrap().get("zzz").unwrap()
}

struct Fixture {
    _tmp: TempDir,
    cache_dir: PathBuf,
    mods_dir: PathBuf,
    ws: Workspace,
    mods: RootId,
    db: Db,
}

impl Fixture {
    async fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("cache");
        let mods_dir = tmp.path().join("XXMI").join("ZZMI").join("Mods");
        fs::create_dir_all(&mods_dir).unwrap();
        let mut ws = Workspace::open(&cache_dir).unwrap();
        let mods = ws.add_mods_root(&mods_dir).unwrap();
        Self {
            _tmp: tmp,
            cache_dir,
            mods_dir,
            ws,
            mods,
            db: Db::open_in_memory().await.unwrap(),
        }
    }

    fn put_archive(&self, name: &str, bytes: &[u8]) -> ScopedPath {
        fs::write(self.cache_dir.join("downloads").join(name), bytes).unwrap();
        self.ws
            .guard()
            .path(self.ws.cache_root(), &format!("downloads/{name}"))
            .unwrap()
    }

    fn analyze(&self, files: &[(&str, &[u8])]) -> Analysis {
        let archive = self.put_archive("test.zip", &zip_of(files));
        analyze(&self.ws, zzz(), &archive).unwrap()
    }

    async fn install(
        &self,
        analysis: &Analysis,
        options: InstallOptions,
    ) -> Result<Vec<gmm_core::InstalledFolder>, CoreError> {
        install_analyzed(
            &self.db,
            &self.ws,
            self.mods,
            zzz(),
            &meta("Ellen - Maid", 652_373),
            analysis,
            &options,
            |_| {},
        )
        .await
    }

    fn gmm(&self) -> PathBuf {
        self.mods_dir.join("GMM")
    }

    /// Every file under `Mods/`, relative, with `/` separators.
    fn tree(&self) -> BTreeSet<String> {
        fn walk(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(base, &path, out);
                } else {
                    let rel = path.strip_prefix(base).unwrap().to_string_lossy();
                    out.insert(rel.replace('\\', "/"));
                }
            }
        }
        let mut out = BTreeSet::new();
        walk(&self.mods_dir, &self.mods_dir, &mut out);
        out
    }

    fn staging_is_empty(&self) -> bool {
        let staging = self.gmm().join("DISABLED .staging");
        !staging.exists() || fs::read_dir(staging).unwrap().next().is_none()
    }
}

fn meta(name: &str, id: u64) -> ModMeta {
    ModMeta {
        name: name.to_owned(),
        gb_mod_id: Some(id),
        gb_file_id: Some(1_400_001),
        file_name: Some("ellen.zip".to_owned()),
        file_date: Some(1_790_000_000),
        version: Some("1.0".to_owned()),
        archive_sha256: "ab".repeat(32),
    }
}

fn named(names: &[&str]) -> InstallOptions {
    InstallOptions {
        variants: VariantChoice::Named(names.iter().map(|s| (*s).to_owned()).collect()),
        ..InstallOptions::default()
    }
}

#[tokio::test]
async fn single_mod_in_a_subfolder() {
    let f = Fixture::new().await;
    let a = f.analyze(&[
        ("README.txt", b"read me"),
        ("Ellen Maid/Ellen.ini", INI),
        ("Ellen Maid/Textures/Body.dds", b"DDS data"),
        // A toggle file further down belongs to the same mod.
        ("Ellen Maid/Extras/Hat.ini", INI),
    ]);
    assert_eq!(a.kind.as_deref(), Some("migoto"));
    assert_eq!(a.variants.len(), 1);
    assert_eq!(a.variants[0].name, "");
    assert_eq!(a.variants[0].root.to_string(), "Ellen Maid");

    let installed = f.install(&a, InstallOptions::default()).await.unwrap();
    assert_eq!(installed.len(), 1);
    let folder = &installed[0];
    assert_eq!(folder.folder_name, "Ellen - Maid [gb-652373]");
    assert_eq!(folder.files, 3);
    assert_eq!(folder.variant, None);

    let p = "GMM/Ellen - Maid [gb-652373]";
    assert_eq!(
        f.tree(),
        BTreeSet::from([
            format!("{p}/Ellen.ini"),
            format!("{p}/Extras/Hat.ini"),
            format!("{p}/Textures/Body.dds"),
            format!("{p}/gmm.json"),
        ]),
        "README stays out; nothing lands outside Mods/GMM"
    );
    assert!(f.staging_is_empty());

    let sidecar: Sidecar =
        serde_json::from_slice(&fs::read(f.mods_dir.join(p).join("gmm.json")).unwrap()).unwrap();
    assert_eq!(sidecar.game, "zzz");
    assert_eq!(sidecar.gb_mod_id, Some(652_373));
    assert_eq!(sidecar.gb_file_id, Some(1_400_001));
    assert_eq!(sidecar.archive_sha256, "ab".repeat(32));

    let row = f.db.installed_mod(folder.id).await.unwrap().unwrap();
    assert_eq!(row.folder_name, "Ellen - Maid [gb-652373]");
    assert!(row.enabled);
    assert_eq!(row.size_bytes, folder.bytes);
    assert!(row.scan_report_json.is_some());

    a.cleanup(&f.ws);
    assert!(
        fs::read_dir(f.cache_dir.join("extract"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn files_at_the_archive_root() {
    let f = Fixture::new().await;
    let a = f.analyze(&[("Mod.ini", INI), ("Body.buf", b"buffer")]);
    assert_eq!(a.variants.len(), 1);
    assert!(a.variants[0].root.is_root());
    let installed = f.install(&a, InstallOptions::default()).await.unwrap();
    assert_eq!(installed[0].files, 2);
}

#[tokio::test]
async fn variants_must_be_chosen() {
    let f = Fixture::new().await;
    let a = f.analyze(&[
        ("Pack/Variant A/a.ini", INI),
        ("Pack/Variant A/a.dds", b"a"),
        ("Pack/Variant B/b.ini", INI),
        ("Pack/Variant B/b.dds", b"b"),
        ("Pack/preview.png", b"png"),
    ]);
    let names: Vec<&str> = a.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Variant A", "Variant B"]);

    match f.install(&a, InstallOptions::default()).await {
        Err(CoreError::ChooseVariant(list)) => {
            assert_eq!(list, ["1. Variant A", "2. Variant B"]);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        f.install(&a, named(&["Variant C"])).await,
        Err(CoreError::UnknownVariant { .. })
    ));
    assert!(
        !f.gmm()
            .join("Ellen - Maid [gb-652373] (Variant B)")
            .exists()
    );

    // By number, then by name (case-insensitive): each its own folder.
    let first = f.install(&a, named(&["2"])).await.unwrap();
    assert_eq!(first[0].folder_name, "Ellen - Maid [gb-652373] (Variant B)");
    assert_eq!(first[0].variant.as_deref(), Some("Variant B"));
    let second = f.install(&a, named(&["variant a"])).await.unwrap();
    assert_eq!(
        second[0].folder_name,
        "Ellen - Maid [gb-652373] (Variant A)"
    );
    assert!(
        f.gmm()
            .join("Ellen - Maid [gb-652373] (Variant A)/a.ini")
            .exists()
    );
    assert_eq!(f.db.installed_mods(Some("zzz")).await.unwrap().len(), 2);
}

#[tokio::test]
async fn all_variants_at_once() {
    let f = Fixture::new().await;
    let a = f.analyze(&[
        ("Red/Long Hair/m.ini", INI),
        ("Red/Short Hair/m.ini", INI),
        ("Blue/m.ini", INI),
    ]);
    let names: Vec<&str> = a.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Blue", "Red - Long Hair", "Red - Short Hair"]);
    let installed = f
        .install(
            &a,
            InstallOptions {
                variants: VariantChoice::All,
                ..InstallOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(installed.len(), 3);
    assert!(
        f.gmm()
            .join("Ellen - Maid [gb-652373] (Red - Long Hair)/m.ini")
            .exists()
    );
}

#[tokio::test]
async fn ignored_inis_and_non_mods() {
    let f = Fixture::new().await;
    // Only files 3DMigoto would load count: DISABLED* and XXMI's own d3dx.ini don't.
    let a = f.analyze(&[
        ("DISABLED old/x.ini", INI),
        ("Main/DISABLED_toggle.ini", INI),
        ("Main/d3dx.ini", INI),
        ("Main/notes.txt", b"hi"),
    ]);
    assert_eq!(a.kind, None);
    assert!(matches!(
        f.install(&a, InstallOptions::default()).await,
        Err(CoreError::NotAMod { .. })
    ));

    let a = f.analyze(&[("textures/a.dds", b"x"), ("config.ini", b"[Constants]\n")]);
    assert_eq!(a.kind, None);
    assert!(a.variants.is_empty());
}

#[tokio::test]
async fn blocked_files_stop_the_install_unless_skipped() {
    let f = Fixture::new().await;
    let a = f.analyze(&[
        ("Mod/m.ini", INI),
        ("Mod/tool.exe", b"MZ\x90\x00"),
        ("Mod/sneaky.png", b"MZ\x90\x00"),
    ]);
    match f.install(&a, InstallOptions::default()).await {
        Err(CoreError::Blocked(files)) => {
            assert_eq!(files, ["Mod/sneaky.png", "Mod/tool.exe"]);
        }
        other => panic!("{other:?}"),
    }
    assert!(f.tree().is_empty(), "nothing was written");

    let installed = f
        .install(
            &a,
            InstallOptions {
                skip_blocked: true,
                ..InstallOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(installed[0].skipped, ["sneaky.png", "tool.exe"]);
    let p = "GMM/Ellen - Maid [gb-652373]";
    assert_eq!(
        f.tree(),
        BTreeSet::from([format!("{p}/gmm.json"), format!("{p}/m.ini")])
    );
}

#[tokio::test]
async fn blocked_files_outside_the_chosen_mod_dont_matter() {
    let f = Fixture::new().await;
    let a = f.analyze(&[("Mod/m.ini", INI), ("Installer.exe", b"MZ\x90\x00")]);
    assert!(a.scan.is_blocked());
    let installed = f.install(&a, InstallOptions::default()).await.unwrap();
    assert!(installed[0].skipped.is_empty());
}

#[tokio::test]
async fn existing_folders_are_never_overwritten() {
    let f = Fixture::new().await;
    let a = f.analyze(&[("Mod/m.ini", INI)]);
    f.install(&a, InstallOptions::default()).await.unwrap();
    assert!(matches!(
        f.install(&a, InstallOptions::default()).await,
        Err(CoreError::AlreadyInstalled { .. })
    ));

    // A folder GMM doesn't know about, enabled or disabled, is left alone.
    let other = Fixture::new().await;
    fs::create_dir_all(other.gmm().join("DISABLED Ellen - Maid [gb-652373]")).unwrap();
    fs::write(
        other
            .gmm()
            .join("DISABLED Ellen - Maid [gb-652373]/mine.ini"),
        b"x",
    )
    .unwrap();
    let a = other.analyze(&[("Mod/m.ini", INI)]);
    assert!(matches!(
        other.install(&a, InstallOptions::default()).await,
        Err(CoreError::FolderTaken { .. })
    ));
    assert_eq!(
        other.tree(),
        BTreeSet::from(["GMM/DISABLED Ellen - Maid [gb-652373]/mine.ini".to_owned()])
    );
}

#[tokio::test]
async fn hostile_mod_names_become_safe_folder_names() {
    let f = Fixture::new().await;
    let a = f.analyze(&[("m.ini", INI)]);
    let installed = install_analyzed(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &meta("DISABLED ..\\..\\Windows: <evil>", 7),
        &a,
        &InstallOptions::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(installed[0].folder_name, "Windows evil [gb-7]");
    assert!(f.gmm().join("Windows evil [gb-7]/m.ini").exists());
}

// ---- end to end against a mock GameBanana ----------------------------------------------

fn net() -> Client {
    Client::new(NetConfig {
        allowlist: Allowlist::with_loopback_for_tests(),
        rate_limit: RateLimit {
            per_second: 1000.0,
            burst: 1000,
        },
        max_backoff: Duration::from_millis(10),
        ..NetConfig::production()
    })
    .unwrap()
}

async fn mock_gamebanana(game_id: u64, av: &str, archive: &[u8]) -> (MockServer, GameBanana) {
    let server = MockServer::start().await;
    let profile = serde_json::json!({
        "_idRow": 900001,
        "_sName": "Test Outfit A",
        "_aGame": { "_idRow": game_id, "_sName": "Some Game" },
        "_sVersion": "2.0",
    });
    let files = serde_json::json!({
        "_aFiles": [{
            "_idRow": 1400001,
            "_sFile": "test_outfit.zip",
            "_nFilesize": archive.len(),
            "_tsDateAdded": 1790000000,
            "_sDownloadUrl": format!("{}/dl/1400001", server.uri()),
            "_sMd5Checksum": "",
            "_sAvResult": av,
        }]
    });
    Mock::given(path("/apiv11/Mod/900001/ProfilePage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(profile))
        .mount(&server)
        .await;
    Mock::given(path("/apiv11/Mod/900001/DownloadPage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(files))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dl/1400001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive.to_vec()))
        .mount(&server)
        .await;
    let base = Url::parse(&format!("{}/apiv11/", server.uri())).unwrap();
    let gb = GameBanana::with_base_url(net(), Arc::new(NoCache), base);
    (server, gb)
}

fn request() -> InstallRequest {
    InstallRequest {
        mod_id: 900001,
        ..InstallRequest::default()
    }
}

#[tokio::test]
async fn end_to_end_install_and_cache_reuse() {
    let f = Fixture::new().await;
    let archive = zip_of(&[("Outfit/outfit.ini", INI), ("Outfit/body.dds", b"dds")]);
    let (_server, gb) = mock_gamebanana(19567, "clean", &archive).await;
    let mut events = Vec::new();

    let report = install(&gb, &net(), &f.db, &f.ws, f.mods, zzz(), &request(), |e| {
        events.push(e);
    })
    .await
    .unwrap();
    assert_eq!(report.file_id, 1_400_001);
    assert!(!report.from_cache);
    assert_eq!(report.installed[0].folder_name, "Test Outfit A [gb-900001]");
    assert!(events.contains(&gmm_core::InstallEvent::Extracting));
    assert!(
        f.gmm()
            .join("Test Outfit A [gb-900001]/outfit.ini")
            .exists()
    );
    let row =
        f.db.installed_mod(report.installed[0].id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        row.version.as_deref(),
        Some("2.0"),
        "mod version when the file has none"
    );
    assert_eq!(
        row.archive_sha256.as_deref(),
        Some(report.archive_sha256.as_str())
    );

    // The archive is cached and recorded; fetching again doesn't download.
    let cached =
        f.db.cached_download_for_file(1_400_001)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(cached.sha256, report.archive_sha256);
    let again = fetch_archive(&gb, &net(), &f.db, &f.ws, 900001, None, |_| {})
        .await
        .unwrap();
    assert!(again.from_cache);
    assert_eq!(again.sha256, report.archive_sha256);
    assert!(
        fs::read_dir(f.cache_dir.join("extract"))
            .unwrap()
            .next()
            .is_none(),
        "extraction folder removed"
    );
}

#[tokio::test]
async fn refuses_other_games_and_unscanned_files() {
    let f = Fixture::new().await;
    let archive = zip_of(&[("m.ini", INI)]);

    let (_server, gb) = mock_gamebanana(20357, "clean", &archive).await;
    assert!(matches!(
        install(&gb, &net(), &f.db, &f.ws, f.mods, zzz(), &request(), |_| {}).await,
        Err(CoreError::WrongGame { found: 20357, .. })
    ));

    let (_server, gb) = mock_gamebanana(19567, "", &archive).await;
    assert!(matches!(
        install(&gb, &net(), &f.db, &f.ws, f.mods, zzz(), &request(), |_| {}).await,
        Err(CoreError::NoCleanFile(900001))
    ));
    assert!(f.tree().is_empty());
    assert!(
        fs::read_dir(f.cache_dir.join("downloads"))
            .unwrap()
            .next()
            .is_none(),
        "nothing downloaded"
    );
}

#[tokio::test]
async fn stale_temporary_files_are_swept_on_open() {
    let tmp = tempfile::tempdir().unwrap();
    let extract = tmp.path().join("cache").join("extract");
    fs::create_dir_all(&extract).unwrap();
    let old = extract.join("old-run.bin");
    fs::write(&old, b"x").unwrap();
    fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(2 * 60 * 60))
        .unwrap();
    fs::write(extract.join("current.bin"), b"x").unwrap();

    Workspace::open(&tmp.path().join("cache")).unwrap();
    assert!(!old.exists(), "older than an hour: removed");
    assert!(extract.join("current.bin").exists(), "recent: kept");
}

// ---- library: list, toggle, uninstall ---------------------------------------------------

/// Deletes instead of using the real Recycle Bin, and remembers what it was asked to do.
#[derive(Default)]
struct FakeTrash(std::sync::Mutex<Vec<String>>);

impl gmm_core::Trash for FakeTrash {
    fn trash(&self, guard: &gmm_fsguard::Guard, path: &ScopedPath) -> Result<(), CoreError> {
        self.0.lock().unwrap().push(path.rel().to_string());
        if guard.metadata(path)?.is_dir() {
            guard.remove_dir_all(path)?;
        } else {
            guard.remove_file(path)?;
        }
        Ok(())
    }
}

async fn installed_one(f: &Fixture) -> i64 {
    let a = f.analyze(&[("Mod/m.ini", INI)]);
    let id = f.install(&a, InstallOptions::default()).await.unwrap()[0].id;
    a.cleanup(&f.ws);
    id
}

#[tokio::test]
async fn toggle_renames_with_the_disabled_prefix() {
    let f = Fixture::new().await;
    let id = installed_one(&f).await;
    let on = f.gmm().join("Ellen - Maid [gb-652373]");
    let off = f.gmm().join("DISABLED Ellen - Maid [gb-652373]");

    let t = gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, false)
        .await
        .unwrap();
    assert!(t.changed && !t.enabled);
    assert!(!on.exists() && off.join("m.ini").exists());
    assert!(!f.db.installed_mod(id).await.unwrap().unwrap().enabled);

    let again = gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, false)
        .await
        .unwrap();
    assert!(!again.changed);

    gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, true)
        .await
        .unwrap();
    assert!(on.join("m.ini").exists() && !off.exists());
    assert!(f.db.installed_mod(id).await.unwrap().unwrap().enabled);

    let wuwa = Registry::builtin().unwrap().get("wuwa").unwrap();
    assert!(matches!(
        gmm_core::set_enabled(&f.db, &f.ws, f.mods, wuwa, id, false).await,
        Err(CoreError::NoSuchMod { .. })
    ));
}

#[tokio::test]
async fn library_follows_the_disk_and_lists_unmanaged_mods() {
    let f = Fixture::new().await;
    let id = installed_one(&f).await;
    // Renamed by hand, outside GMM.
    fs::rename(
        f.gmm().join("Ellen - Maid [gb-652373]"),
        f.gmm().join("DISABLED Ellen - Maid [gb-652373]"),
    )
    .unwrap();
    fs::create_dir_all(f.mods_dir.join("Phoenix")).unwrap();
    fs::create_dir_all(f.mods_dir.join("DISABLED Old Outfit")).unwrap();
    fs::write(f.mods_dir.join("loose.ini"), INI).unwrap();
    fs::write(f.mods_dir.join("readme.txt"), b"x").unwrap();
    fs::create_dir_all(f.gmm().join("Stranger")).unwrap();
    fs::create_dir_all(f.gmm().join("DISABLED .staging").join("leftover")).unwrap();

    let lib = gmm_core::library(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();
    assert_eq!(lib.managed.len(), 1);
    assert_eq!(lib.managed[0].disk, gmm_core::DiskState::Disabled);
    assert!(!lib.managed[0].row.enabled);
    assert!(
        !f.db.installed_mod(id).await.unwrap().unwrap().enabled,
        "database corrected"
    );
    let unmanaged: Vec<(&str, bool)> = lib
        .unmanaged
        .iter()
        .map(|u| (u.path.as_str(), u.enabled))
        .collect();
    assert_eq!(
        unmanaged,
        [
            ("DISABLED Old Outfit", false),
            ("Phoenix", true),
            ("loose.ini", true),
            ("GMM/Stranger", true),
        ]
    );
}

#[tokio::test]
async fn uninstall_trashes_the_folder_and_forgets_the_mod() {
    let f = Fixture::new().await;
    let id = installed_one(&f).await;
    gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, false)
        .await
        .unwrap();
    let trash = FakeTrash::default();
    let out = gmm_core::uninstall(&f.db, &f.ws, f.mods, zzz(), id, &trash)
        .await
        .unwrap();
    assert!(out.trashed);
    assert_eq!(
        *trash.0.lock().unwrap(),
        ["GMM/DISABLED Ellen - Maid [gb-652373]"]
    );
    assert_eq!(f.db.installed_mod(id).await.unwrap(), None);
    assert!(f.tree().is_empty());
}

#[tokio::test]
async fn missing_folders_are_reported_not_guessed() {
    let f = Fixture::new().await;
    let id = installed_one(&f).await;
    fs::remove_dir_all(f.gmm().join("Ellen - Maid [gb-652373]")).unwrap();

    let lib = gmm_core::library(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();
    assert_eq!(lib.managed[0].disk, gmm_core::DiskState::Missing);
    assert!(matches!(
        gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, false).await,
        Err(CoreError::ModFolderMissing(_))
    ));
    let trash = FakeTrash::default();
    let out = gmm_core::uninstall(&f.db, &f.ws, f.mods, zzz(), id, &trash)
        .await
        .unwrap();
    assert!(!out.trashed, "only the record is removed");
    assert!(trash.0.lock().unwrap().is_empty());
    assert_eq!(f.db.installed_mod(id).await.unwrap(), None);
}

// ---- conflicts ---------------------------------------------------------------------------

fn ini_for(hash: &str, extra: &str) -> Vec<u8> {
    format!("[TextureOverrideBody]\nhash = {hash}\n{extra}\n").into_bytes()
}

async fn install_named(f: &Fixture, name: &str, id: u64, files: &[(&str, &[u8])]) -> i64 {
    let a = f.analyze(files);
    let out = install_analyzed(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &meta(name, id),
        &a,
        &InstallOptions::default(),
        |_| {},
    )
    .await
    .unwrap();
    a.cleanup(&f.ws);
    out[0].id
}

#[tokio::test]
async fn conflicts_between_enabled_mods() {
    let f = Fixture::new().await;
    let a = install_named(
        &f,
        "Outfit A",
        1,
        &[("m.ini", &ini_for("aaaa1111", "ib = R"))],
    )
    .await;
    let b = install_named(
        &f,
        "Outfit B",
        2,
        &[("x/m.ini", &ini_for("AAAA1111", "ps-t0 = R"))],
    )
    .await;
    // Same hash but it only watches: not a conflict.
    install_named(
        &f,
        "Watcher",
        3,
        &[("m.ini", &ini_for("aaaa1111", "$x = 1"))],
    )
    .await;
    // A hand-installed mod replacing the same thing.
    fs::create_dir_all(f.mods_dir.join("Manual")).unwrap();
    fs::write(
        f.mods_dir.join("Manual/m.ini"),
        ini_for("aaaa1111", "vb0 = R"),
    )
    .unwrap();
    // Disabled things are never read.
    fs::create_dir_all(f.mods_dir.join("DISABLED Old")).unwrap();
    fs::write(
        f.mods_dir.join("DISABLED Old/m.ini"),
        ini_for("aaaa1111", "ib = R"),
    )
    .unwrap();

    let report = gmm_core::conflicts(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();
    assert_eq!(report.mods_checked, 4);
    let pairs: Vec<(String, String)> = report
        .conflicts
        .iter()
        .map(|c| (c.a.label(), c.b.label()))
        .collect();
    assert_eq!(
        pairs,
        [
            (
                format!("#{a} Outfit A [gb-1]"),
                format!("#{b} Outfit B [gb-2]")
            ),
            (
                format!("#{a} Outfit A [gb-1]"),
                "Manual (not installed by GMM)".to_owned()
            ),
            (
                format!("#{b} Outfit B [gb-2]"),
                "Manual (not installed by GMM)".to_owned()
            ),
        ]
    );
    assert_eq!(report.conflicts[0].hashes, ["aaaa1111"]);

    // Disabling B removes its conflicts; checking B before enabling it shows them again.
    gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), b, false)
        .await
        .unwrap();
    let report = gmm_core::conflicts(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();
    assert_eq!(report.conflicts.len(), 1);
    let before = gmm_core::conflicts_for(&f.db, &f.ws, f.mods, zzz(), b)
        .await
        .unwrap();
    assert_eq!(before.conflicts.len(), 2, "with A and with Manual");
    assert!(
        before
            .conflicts
            .iter()
            .all(|c| c.a.label().contains("Outfit B") || c.b.label().contains("Outfit B"))
    );
}

#[tokio::test]
async fn different_parts_and_libraries_dont_conflict() {
    let f = Fixture::new().await;
    install_named(
        &f,
        "Top",
        1,
        &[(
            "m.ini",
            &ini_for("bbbb2222", "match_first_index = 0\nib = R"),
        )],
    )
    .await;
    install_named(
        &f,
        "Bottom",
        2,
        &[(
            "m.ini",
            &ini_for("bbbb2222", "match_first_index = 9000\nib = R"),
        )],
    )
    .await;
    let report = gmm_core::conflicts(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();
    assert!(report.conflicts.is_empty(), "{:?}", report.conflicts);

    // A manifest that lists a shared library namespace.
    let toml = include_str!("../../gmm-adapters/games/zzz.toml").replace(
        "ignore_namespaces = []",
        r"ignore_namespaces = ['Shared\Lib']",
    );
    let registry = Registry::from_sources(&[("zzz.toml", &toml)]).unwrap();
    let game = registry.get("zzz").unwrap();
    fs::create_dir_all(f.mods_dir.join("Lib")).unwrap();
    fs::write(
        f.mods_dir.join("Lib/lib.ini"),
        format!(
            "namespace = shared\\lib\n{}",
            String::from_utf8(ini_for("bbbb2222", "ib = R")).unwrap()
        ),
    )
    .unwrap();
    let report = gmm_core::conflicts(&f.db, &f.ws, f.mods, game)
        .await
        .unwrap();
    assert!(report.conflicts.is_empty(), "{:?}", report.conflicts);
    assert_eq!(report.libraries.len(), 1);
}

// ---- storage -----------------------------------------------------------------------------

#[tokio::test]
async fn storage_measures_mods_cache_and_free_space() {
    let f = Fixture::new().await;
    let id = install_named(
        &f,
        "Big",
        1,
        &[
            ("m.ini", &ini_for("cccc3333", "ib = R")),
            ("t.dds", &[0u8; 5000]),
        ],
    )
    .await;
    fs::create_dir_all(f.mods_dir.join("Manual")).unwrap();
    fs::write(f.mods_dir.join("Manual/a.dds"), [0u8; 300]).unwrap();
    fs::write(f.mods_dir.join("notes.txt"), [0u8; 7]).unwrap();
    // The recorded size drifts when someone adds files by hand; storage corrects it.
    fs::write(f.gmm().join("Big [gb-1]/extra.dds"), [0u8; 1000]).unwrap();

    let s = gmm_core::game_storage(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();
    assert_eq!(s.managed.len(), 1);
    let sidecar = fs::metadata(f.gmm().join("Big [gb-1]/gmm.json"))
        .unwrap()
        .len();
    let ini = ini_for("cccc3333", "ib = R").len() as u64;
    assert_eq!(s.managed_total.bytes, 5000 + 1000 + ini + sidecar);
    assert_eq!(s.managed_total.files, 4);
    assert_eq!(s.unmanaged_total.bytes, 300);
    assert_eq!(s.other.bytes, 7);
    assert!(s.free_bytes.is_some_and(|b| b > 0));
    assert_eq!(
        f.db.installed_mod(id).await.unwrap().unwrap().size_bytes,
        s.managed_total.bytes
    );

    let c = gmm_core::cache_storage(&f.db, &f.ws).await.unwrap();
    assert!(c.downloads.bytes > 0, "the test archive sits in downloads/");
    assert_eq!(c.extract.bytes, 0, "extraction folders were cleaned up");

    let trash = FakeTrash::default();
    let cleared = gmm_core::clear_downloads(&f.db, &f.ws, &trash)
        .await
        .unwrap();
    assert_eq!(cleared.archives, 1);
    assert_eq!(cleared.bytes, c.downloads.bytes);
    assert_eq!(
        gmm_core::cache_storage(&f.db, &f.ws)
            .await
            .unwrap()
            .downloads
            .bytes,
        0
    );
    assert!(
        f.gmm().join("Big [gb-1]/m.ini").exists(),
        "installed mods stay"
    );
}
