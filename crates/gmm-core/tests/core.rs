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
    Analysis, CoreError, InstallOptions, InstallRequest, ModMeta, Sidecar, StopSignal,
    VariantChoice, Workspace, analyze, fetch_archive, install, install_analyzed,
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
        listing: gmm_db::ModListing::default(),
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
        "_aCategory": { "_sName": "Ellen", "_sProfileUrl": "https://gamebanana.com/mods/cats/43004" },
        "_aSuperCategory": { "_sName": "Character Skins", "_sProfileUrl": "https://gamebanana.com/mods/cats/30305" },
        "_aPreviewMedia": { "_aImages": [{
            "_sBaseUrl": "https://images.gamebanana.com/img/ss/mods", "_sFile": "a.jpg",
            "_sFile220": "220-90_a.jpg", "_wFile220": 220, "_hFile220": 124,
            "_sFile530": "530-90_a.jpg", "_wFile530": 530, "_hFile530": 298,
        }] },
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
async fn the_same_file_fetched_twice_at_once_downloads_once() {
    // The install confirmation and the queue (or a double-run effect) can ask together.
    let f = Fixture::new().await;
    let archive = zip_of(&[("Outfit/outfit.ini", INI)]);
    let (_server, gb) = mock_gamebanana(19567, "clean", &archive).await;
    let stop = StopSignal::new();
    let net = net();
    let (a, b) = tokio::join!(
        fetch_archive(&gb, &net, &f.db, &f.ws, 900001, None, &stop, |_| {}),
        fetch_archive(&gb, &net, &f.db, &f.ws, 900001, None, &stop, |_| {}),
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.sha256, b.sha256);
    assert!(
        a.from_cache != b.from_cache,
        "one downloads, the other finds it in the cache"
    );
}

#[tokio::test]
async fn end_to_end_install_and_cache_reuse() {
    let f = Fixture::new().await;
    let archive = zip_of(&[("Outfit/outfit.ini", INI), ("Outfit/body.dds", b"dds")]);
    let (_server, gb) = mock_gamebanana(19567, "clean", &archive).await;
    let mut events = Vec::new();

    let stop = StopSignal::new();
    let report = install(
        &gb,
        &net(),
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &request(),
        &stop,
        |e| {
            events.push(e);
        },
    )
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
    assert_eq!(
        row.listing,
        gmm_db::ModListing {
            root_category_id: Some(30_305),
            root_category: Some("Character Skins".to_owned()),
            category_id: Some(43_004),
            category: Some("Ellen".to_owned()),
            sensitive: Some(false),
            preview_url: Some("https://images.gamebanana.com/img/ss/mods/530-90_a.jpg".to_owned()),
        },
        "the listing from the mod page"
    );

    // The archive is cached and recorded; fetching again doesn't download.
    let cached =
        f.db.cached_download_for_file(1_400_001)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(cached.sha256, report.archive_sha256);
    let again = fetch_archive(&gb, &net(), &f.db, &f.ws, 900001, None, &stop, |_| {})
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
        install(
            &gb,
            &net(),
            &f.db,
            &f.ws,
            f.mods,
            zzz(),
            &request(),
            &StopSignal::new(),
            |_| {}
        )
        .await,
        Err(CoreError::WrongGame { found: 20357, .. })
    ));

    let (_server, gb) = mock_gamebanana(19567, "", &archive).await;
    assert!(matches!(
        install(
            &gb,
            &net(),
            &f.db,
            &f.ws,
            f.mods,
            zzz(),
            &request(),
            &StopSignal::new(),
            |_| {}
        )
        .await,
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
struct FakeRemover(std::sync::Mutex<Vec<String>>);

impl gmm_core::Remover for FakeRemover {
    fn remove(&self, guard: &gmm_fsguard::Guard, path: &ScopedPath) -> Result<(), CoreError> {
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
async fn uninstall_deletes_the_folder_and_forgets_the_mod() {
    let f = Fixture::new().await;
    let id = installed_one(&f).await;
    gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, false)
        .await
        .unwrap();
    let remover = FakeRemover::default();
    let out = gmm_core::uninstall(&f.db, &f.ws, f.mods, zzz(), id, &remover)
        .await
        .unwrap();
    assert!(out.removed);
    assert_eq!(
        *remover.0.lock().unwrap(),
        ["GMM/DISABLED Ellen - Maid [gb-652373]"]
    );
    assert_eq!(f.db.installed_mod(id).await.unwrap(), None);
    assert!(f.tree().is_empty());
}

#[tokio::test]
async fn uninstalling_the_last_version_deletes_its_cached_archives() {
    let f = Fixture::new().await;
    let id = installed_one(&f).await;
    let gb = f.db.installed_mod(id).await.unwrap().unwrap().gb_mod_id;
    let downloads = f.cache_dir.join("downloads");
    fs::create_dir_all(&downloads).unwrap();
    for (sha, name) in [("a1", "1-new.zip"), ("b2", "2-old.zip")] {
        fs::write(downloads.join(name), b"PK").unwrap();
        f.db.record_download(&gmm_db::NewCachedDownload {
            sha256: sha.into(),
            gb_mod_id: gb,
            gb_file_id: Some(1),
            path_rel: format!("downloads/{name}"),
            size_bytes: 2,
        })
        .await
        .unwrap();
    }
    fs::write(downloads.join("9-other.zip"), b"PK").unwrap();

    let remover = FakeRemover::default();
    let out = gmm_core::uninstall(&f.db, &f.ws, f.mods, zzz(), id, &remover)
        .await
        .unwrap();
    assert_eq!(out.archives_removed, 2);
    assert!(!downloads.join("1-new.zip").exists() && !downloads.join("2-old.zip").exists());
    assert!(
        downloads.join("9-other.zip").exists(),
        "other mods' archives stay"
    );
    assert!(f.db.cached_downloads().await.unwrap().is_empty());
}

/// 3DMigoto drops a switched-off mod's keys from d3dx_user.ini; GMM puts them back. While
/// the game runs (the test machine may have ZZZ open) the lines wait as pending instead.
#[tokio::test]
async fn switching_off_and_on_keeps_the_saved_in_game_toggles() {
    let f = Fixture::new().await;
    let id = installed_one(&f).await;
    let user_ini = f.mods_dir.parent().unwrap().join("d3dx_user.ini");
    let ours = r"$\mods\gmm\ellen - maid [gb-652373]\m.ini\swapvar = 2";
    let xxmi = r"$\zzmiv1\shown = 1";
    let with_ours = format!("; generated\r\n[Constants]\r\n{xxmi}\r\n{ours}\r\n");
    let without = format!("; generated\r\n[Constants]\r\n{xxmi}\r\n");
    fs::write(&user_ini, &with_ours).unwrap();

    gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, false)
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(&user_ini).unwrap(),
        with_ours,
        "switching off doesn't edit the file"
    );
    // The game saves without the mod: its keys are gone.
    fs::write(&user_ini, &without).unwrap();

    gmm_core::set_enabled(&f.db, &f.ws, f.mods, zzz(), id, true)
        .await
        .unwrap();
    if gmm_xxmi::game_running(zzz()) {
        assert_eq!(
            fs::read_to_string(&user_ini).unwrap(),
            without,
            "never while it runs"
        );
        assert_eq!(
            f.db.pending_toggles("zzz").await.unwrap().len(),
            1,
            "kept for later"
        );
        return;
    }
    assert_eq!(fs::read_to_string(&user_ini).unwrap(), with_ours);
    let backups: Vec<_> = fs::read_dir(f.cache_dir.join("backups").join("d3dx_user"))
        .unwrap()
        .collect();
    assert_eq!(backups.len(), 1, "the old file was kept first");

    // Nothing else in the importer folder was written.
    let mut names: Vec<String> = fs::read_dir(f.mods_dir.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Mods", "d3dx_user.ini"]);
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
    let remover = FakeRemover::default();
    let out = gmm_core::uninstall(&f.db, &f.ws, f.mods, zzz(), id, &remover)
        .await
        .unwrap();
    assert!(!out.removed, "only the record is removed");
    assert!(remover.0.lock().unwrap().is_empty());
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

    let remover = FakeRemover::default();
    let cleared = gmm_core::clear_downloads(&f.db, &f.ws, &remover)
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

// ---- launch ------------------------------------------------------------------------------

fn fake_xxmi(root: &Path) -> gmm_xxmi::XxmiInstall {
    fs::create_dir_all(root.join("Resources/Bin")).unwrap();
    fs::write(root.join("Resources/Bin/XXMI Launcher.exe"), b"MZ\x90\x00").unwrap();
    fs::write(root.join("XXMI Launcher Config.json"), b"{}").unwrap();
    gmm_xxmi::open_xxmi(root).unwrap()
}

fn status(state: gmm_xxmi::SetupState) -> gmm_xxmi::GameStatus {
    gmm_xxmi::GameStatus {
        game_id: "zzz".to_owned(),
        game_name: "Zenless Zone Zero".to_owned(),
        state,
        summary: "test".to_owned(),
        steps: Vec::new(),
        notes: Vec::new(),
        detected_installs: Vec::new(),
        xxmi_root: None,
        xxmi_version: None,
        importer: "ZZMI".to_owned(),
        importer_info: None,
        xxmi_game_folder: None,
        xxmi_game_folder_source: None,
        game_launch: None,
        launch_count: None,
        launch_command: None,
    }
}

#[tokio::test]
async fn launch_needs_ready_and_a_confirmed_exe() {
    use gmm_xxmi::SetupState;
    let tmp = tempfile::tempdir().unwrap();
    let xxmi = fake_xxmi(&tmp.path().join("XXMI Launcher"));
    let db = Db::open_in_memory().await.unwrap();

    assert!(matches!(
        gmm_core::plan_launch(&db, &xxmi, &status(SetupState::LaunchModeMismatch), zzz()).await,
        Err(CoreError::NotReady { .. })
    ));

    let mut plan = gmm_core::plan_launch(&db, &xxmi, &status(SetupState::Ready), zzz())
        .await
        .unwrap();
    assert!(
        plan.command
            .exe
            .ends_with("Resources/Bin/XXMI Launcher.exe")
    );
    assert_eq!(plan.command.args, ["--nogui", "--xxmi", "ZZMI"]);
    assert!(!plan.confirmed);
    assert!(matches!(
        gmm_core::start(&db, zzz(), &plan).await,
        Err(CoreError::ExeNotConfirmed(_))
    ));
    assert!(
        db.last_played().await.unwrap().is_empty(),
        "nothing started"
    );

    gmm_core::confirm_exe(&db, &mut plan).await.unwrap();
    let again = gmm_core::plan_launch(&db, &xxmi, &status(SetupState::Ready), zzz())
        .await
        .unwrap();
    assert!(again.confirmed, "remembered");

    // A different XXMI folder must be confirmed again.
    let other = fake_xxmi(&tmp.path().join("Portable XXMI"));
    let moved = gmm_core::plan_launch(&db, &other, &status(SetupState::Ready), zzz())
        .await
        .unwrap();
    assert!(!moved.confirmed);

    // Something that isn't a program anymore is refused before anything runs.
    fs::write(
        tmp.path()
            .join("XXMI Launcher/Resources/Bin/XXMI Launcher.exe"),
        b"#!/bin/sh",
    )
    .unwrap();
    assert!(matches!(
        gmm_core::plan_launch(&db, &xxmi, &status(SetupState::Ready), zzz()).await,
        Err(CoreError::Launch(_))
    ));
}

// ---- mods installed by hand --------------------------------------------------------------

#[tokio::test]
async fn hand_installed_mods_can_be_switched_and_uninstalled() {
    let f = Fixture::new().await;
    let managed = install_named(
        &f,
        "Managed",
        9,
        &[("m.ini", &ini_for("eeee0000", "ib = R"))],
    )
    .await;
    let ini = ini_for("dddd4444", "ib = R");
    for dir in [
        "Phoenix",
        "GMM/Phoenix_duplicate",
        "DISABLED_Old",
        "Dup",
        "GMM/Dup",
    ] {
        fs::create_dir_all(f.mods_dir.join(dir)).unwrap();
        fs::write(f.mods_dir.join(dir).join("m.ini"), &ini).unwrap();
    }
    fs::write(f.mods_dir.join("loose.ini"), b"[Constants]\n").unwrap();
    let lib = gmm_core::library(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();

    // Found by name (any case, with or without DISABLED) or by the listed path; GMM's own
    // mods are never among them.
    let paths = |q: &str| -> Vec<String> {
        gmm_core::find_unmanaged(&lib, q)
            .iter()
            .map(|u| u.path.clone())
            .collect()
    };
    assert_eq!(paths("phoenix_duplicate"), ["GMM/Phoenix_duplicate"]);
    assert_eq!(paths("Phoenix"), ["Phoenix"]);
    assert_eq!(paths("old"), ["DISABLED_Old"]);
    assert_eq!(paths("Dup").len(), 2);
    assert_eq!(paths("GMM/Dup"), ["GMM/Dup"]);
    assert!(paths("Managed [gb-9]").is_empty());
    assert!(matches!(
        gmm_core::one_unmanaged(&lib, zzz(), "Dup"),
        Err(CoreError::AmbiguousUnmanaged { .. })
    ));

    // The app passes the exact listed path, which names one of them (and nothing looser).
    let top = gmm_core::set_unmanaged_enabled_at(&f.db, &f.ws, f.mods, zzz(), "Dup", false)
        .await
        .unwrap();
    assert_eq!(
        (top.from.as_str(), top.to.as_str()),
        ("Dup", "DISABLED Dup")
    );
    assert!(f.gmm().join("Dup/m.ini").exists());
    for loose in ["dup", "DISABLED dup", "Dup"] {
        assert!(matches!(
            gmm_core::set_unmanaged_enabled_at(&f.db, &f.ws, f.mods, zzz(), loose, true).await,
            Err(CoreError::NoSuchUnmanaged { .. })
        ));
    }
    gmm_core::set_unmanaged_enabled_at(&f.db, &f.ws, f.mods, zzz(), "DISABLED Dup", true)
        .await
        .unwrap();
    assert!(f.mods_dir.join("Dup/m.ini").exists());

    // Off and on with XXMI's prefix.
    let off =
        gmm_core::set_unmanaged_enabled(&f.db, &f.ws, f.mods, zzz(), "Phoenix_duplicate", false)
            .await
            .unwrap();
    assert_eq!(
        (off.from.as_str(), off.to.as_str()),
        ("GMM/Phoenix_duplicate", "GMM/DISABLED Phoenix_duplicate")
    );
    assert!(f.gmm().join("DISABLED Phoenix_duplicate/m.ini").exists());
    let again =
        gmm_core::set_unmanaged_enabled(&f.db, &f.ws, f.mods, zzz(), "phoenix_duplicate", false)
            .await
            .unwrap();
    assert!(!again.changed);
    let on = gmm_core::set_unmanaged_enabled(&f.db, &f.ws, f.mods, zzz(), "Old", true)
        .await
        .unwrap();
    assert_eq!(on.to, "Old", "any DISABLED prefix style is removed");
    let loose = gmm_core::set_unmanaged_enabled(&f.db, &f.ws, f.mods, zzz(), "loose.ini", false)
        .await
        .unwrap();
    assert_eq!(loose.to, "DISABLED loose.ini");

    // Before switching the duplicate back on: it would clash with the mods still on.
    let report = gmm_core::conflicts_for_unmanaged(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        "GMM/DISABLED Phoenix_duplicate",
    )
    .await
    .unwrap();
    let partners: Vec<String> = report
        .conflicts
        .iter()
        .map(|c| format!("{} | {}", c.a.label(), c.b.label()))
        .collect();
    assert_eq!(report.conflicts.len(), 4, "{partners:?}"); // Phoenix, Old, Dup, GMM/Dup

    // Uninstall takes only an exact listed path, and goes through the trash.
    let remover = FakeRemover::default();
    assert!(matches!(
        gmm_core::uninstall_unmanaged(&f.db, &f.ws, f.mods, zzz(), "phoenix", &remover).await,
        Err(CoreError::NoSuchUnmanaged { .. })
    ));
    let gone = gmm_core::uninstall_unmanaged(&f.db, &f.ws, f.mods, zzz(), "Phoenix", &remover)
        .await
        .unwrap();
    assert_eq!(gone, "Phoenix");
    assert_eq!(*remover.0.lock().unwrap(), ["Phoenix"]);
    assert!(!f.mods_dir.join("Phoenix").exists());
    assert!(f.gmm().join("Managed [gb-9]/m.ini").exists());
    assert!(f.db.installed_mod(managed).await.unwrap().is_some());
}

// ---- Phase 5: two-step install, versions, update check, stopping ------------------------

const INI_OTHER: &[u8] = b"[TextureOverrideHair]\nhash = ffff0000\nib = ResourceHairIB\n";

#[tokio::test]
async fn installs_switched_off_when_asked() {
    let f = Fixture::new().await;
    let a = f.analyze(&[("Ellen/ellen.ini", INI)]);
    let installed = f
        .install(
            &a,
            InstallOptions {
                start_disabled: true,
                ..InstallOptions::default()
            },
        )
        .await
        .unwrap();
    assert!(
        f.gmm()
            .join("DISABLED Ellen - Maid [gb-652373]/ellen.ini")
            .exists()
    );
    let row = f.db.installed_mod(installed[0].id).await.unwrap().unwrap();
    assert!(!row.enabled);
    let lib = gmm_core::library(&f.db, &f.ws, f.mods, zzz())
        .await
        .unwrap();
    assert_eq!(lib.managed[0].disk, gmm_core::DiskState::Disabled);
}

#[tokio::test]
async fn versions_that_replace_the_same_thing_conflict() {
    let f = Fixture::new().await;
    let a = f.analyze(&[
        ("Pack/A/a.ini", INI),
        ("Pack/B/b.ini", INI),
        ("Pack/C/c.ini", INI_OTHER),
    ]);
    let names: Vec<&str> = a.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["A", "B", "C"]);
    assert_eq!(
        a.version_conflicts,
        [gmm_core::VersionConflict {
            a: 0,
            b: 1,
            hashes: 1
        }]
    );
}

#[tokio::test]
async fn switching_a_version_turns_off_the_ones_it_clashes_with() {
    let f = Fixture::new().await;
    let a = f.analyze(&[
        ("Pack/A/a.ini", INI),
        ("Pack/B/b.ini", INI),
        ("Pack/C/c.ini", INI_OTHER),
    ]);
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
    let id = |name: &str| {
        installed
            .iter()
            .find(|i| i.variant.as_deref() == Some(name))
            .unwrap()
            .id
    };
    let changes = gmm_core::switch_version(&f.db, &f.ws, f.mods, zzz(), id("B"))
        .await
        .unwrap();
    let summary: Vec<(i64, bool)> = changes.iter().map(|c| (c.id, c.enabled)).collect();
    assert_eq!(summary, [(id("B"), true), (id("A"), false)]);
    let on =
        |rows: &[gmm_db::InstalledMod], i: i64| rows.iter().find(|m| m.id == i).unwrap().enabled;
    let rows = f.db.installed_mods(Some("zzz")).await.unwrap();
    assert!(!on(&rows, id("A")));
    assert!(on(&rows, id("B")));
    assert!(on(&rows, id("C")), "a version that doesn't clash stays on");

    // Not a GameBanana version: refused.
    let mut plain = meta("Loose", 1);
    plain.gb_file_id = None;
    let loose = f.analyze(&[("L/l.ini", INI_OTHER)]);
    let one = install_analyzed(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &plain,
        &loose,
        &InstallOptions::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert!(matches!(
        gmm_core::switch_version(&f.db, &f.ws, f.mods, zzz(), one[0].id).await,
        Err(CoreError::NotAVersion { .. })
    ));
}

#[tokio::test]
async fn two_step_install_shows_the_plan_then_installs_from_it() {
    let f = Fixture::new().await;
    let archive = zip_of(&[
        ("Pack/Red/red.ini", INI),
        ("Pack/Blue/blue.ini", INI),
        ("Pack/Blue/tool.exe", b"MZ"),
    ]);
    let (_server, gb) = mock_gamebanana(19567, "clean", &archive).await;
    let stop = StopSignal::new();

    let prepared = gmm_core::prepare(
        &gb,
        &net(),
        &f.db,
        &f.ws,
        zzz(),
        900001,
        None,
        &stop,
        |_| {},
    )
    .await
    .unwrap();
    let plan = gmm_core::install_plan(&f.db, zzz(), &prepared)
        .await
        .unwrap();
    assert_eq!(plan.mod_name, "Test Outfit A");
    assert_eq!(plan.install_to, r"ZZMI\Mods\GMM");
    assert!(!plan.from_cache);
    let names: Vec<&str> = plan.versions.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Blue", "Red"]);
    assert_eq!(plan.versions[0].blocked, ["tool.exe"]);
    assert_eq!(plan.versions[0].folder, "Test Outfit A [gb-900001] (Blue)");
    assert!(plan.versions.iter().all(|v| v.installed.is_none()));
    assert_eq!(plan.version_conflicts.len(), 1);
    assert!(f.tree().is_empty(), "preparing installs nothing");

    let report = gmm_core::install_prepared(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &prepared,
        &InstallOptions {
            variants: VariantChoice::Named(vec!["Red".into()]),
            ..InstallOptions::default()
        },
        |_| {},
    )
    .await
    .unwrap();
    prepared.discard(&f.ws);
    assert_eq!(report.installed.len(), 1);

    // Preparing again reuses the cached archive and marks the installed version.
    let again = gmm_core::prepare(
        &gb,
        &net(),
        &f.db,
        &f.ws,
        zzz(),
        900001,
        None,
        &stop,
        |_| {},
    )
    .await
    .unwrap();
    let plan = gmm_core::install_plan(&f.db, zzz(), &again).await.unwrap();
    again.discard(&f.ws);
    assert!(plan.from_cache);
    assert_eq!(plan.versions[1].installed, Some(report.installed[0].id));
    assert_eq!(plan.versions[0].installed, None);
    assert!(
        fs::read_dir(f.cache_dir.join("extract"))
            .unwrap()
            .next()
            .is_none(),
        "discard removes the extraction folders"
    );
}

#[tokio::test]
async fn a_stopped_install_does_nothing() {
    let f = Fixture::new().await;
    let archive = zip_of(&[("m.ini", INI)]);
    let (_server, gb) = mock_gamebanana(19567, "clean", &archive).await;
    for (stop_how, want) in [
        (
            StopSignal::cancel as fn(&StopSignal),
            gmm_core::StopReason::Cancelled,
        ),
        (StopSignal::pause, gmm_core::StopReason::Paused),
    ] {
        let stop = StopSignal::new();
        stop_how(&stop);
        let result = install(
            &gb,
            &net(),
            &f.db,
            &f.ws,
            f.mods,
            zzz(),
            &request(),
            &stop,
            |_| {},
        )
        .await;
        assert!(matches!(result, Err(CoreError::Stopped(r)) if r == want));
    }
    assert!(f.tree().is_empty());
}

#[tokio::test]
async fn update_check_finds_newer_files() {
    let f = Fixture::new().await;
    let archive = zip_of(&[("m.ini", INI)]);
    let (_server, gb) = mock_gamebanana(19567, "clean", &archive).await;
    let a = f.analyze(&[("m.ini", INI)]);
    let mut old = meta("Test Outfit A", 900001);
    old.gb_file_id = Some(1_399_999);
    old.file_date = Some(1_780_000_000);
    install_analyzed(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &old,
        &a,
        &InstallOptions::default(),
        |_| {},
    )
    .await
    .unwrap();

    let check = gmm_core::check_updates(&gb, &f.db, zzz()).await.unwrap();
    assert_eq!(check.checked, 1);
    assert_eq!(check.updates.len(), 1);
    assert_eq!(check.updates[0].newest.id, 1_400_001);
    assert_eq!(check.updates[0].installed_file_id, Some(1_399_999));
    assert!(check.notes.is_empty());

    // A mod GameBanana no longer has becomes a note, not a failure.
    let mut gone = meta("Gone", 123);
    gone.gb_file_id = Some(5);
    let b = f.analyze(&[("g.ini", INI_OTHER)]);
    install_analyzed(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &gone,
        &b,
        &InstallOptions::default(),
        |_| {},
    )
    .await
    .unwrap();
    let check = gmm_core::check_updates(&gb, &f.db, zzz()).await.unwrap();
    assert_eq!(check.checked, 2);
    assert_eq!(check.updates.len(), 1);
    assert_eq!(check.notes.len(), 1);
}

#[tokio::test]
async fn key_bindings_are_read_from_every_ini_but_disabled_ones() {
    let f = Fixture::new().await;
    let main = b"[TextureOverrideBody]\nhash = 0123abcd\n\n[KeySwap]\nkey = VK_LEFT\nback = VK_RIGHT\ntype = cycle\n";
    let extra = b"[KeyGlasses]\nkey = no_modifiers ctrl 1\ntype = toggle\n";
    let off = b"[KeyIgnored]\nkey = F9\n";
    let a = f.analyze(&[
        ("Ellen/ellen.ini", main),
        ("Ellen/Parts/glasses.ini", extra),
        ("Ellen/DISABLED old/old.ini", off),
    ]);
    let id = f.install(&a, InstallOptions::default()).await.unwrap()[0].id;
    let keys = gmm_core::mod_keybindings(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        gmm_core::ModFolder::Managed(id),
    )
    .await
    .unwrap();
    let names: Vec<(&str, &str)> = keys
        .bindings
        .iter()
        .map(|k| (k.ini.as_str(), k.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [("Parts/glasses.ini", "Glasses"), ("ellen.ini", "Swap")]
    );
    let swap = &keys.bindings[1];
    assert_eq!(swap.keys, ["VK_LEFT"]);
    assert_eq!(swap.back, ["VK_RIGHT"]);
    assert_eq!(swap.kind.as_deref(), Some("cycle"));
    assert!(keys.notes.is_empty());
}

#[tokio::test]
async fn profiles_save_apply_and_share() {
    let f = Fixture::new().await;
    let a = f.analyze(&[
        ("Red/red.ini", INI),
        ("Blue/blue.ini", b"[TextureOverrideHair]\nhash = 99999999\n"),
    ]);
    let ids: Vec<i64> = f
        .install(
            &a,
            InstallOptions {
                variants: VariantChoice::All,
                ..InstallOptions::default()
            },
        )
        .await
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect();
    fs::create_dir_all(f.mods_dir.join("Phoenix")).unwrap();
    fs::write(f.mods_dir.join("Phoenix/phoenix.ini"), INI).unwrap();
    let (red, blue) = (ids[0], ids[1]);
    let (db, ws, mods) = (&f.db, &f.ws, f.mods);

    // Red and Phoenix on, Blue off.
    gmm_core::set_enabled(db, ws, mods, zzz(), blue, false)
        .await
        .unwrap();
    let saved = gmm_core::save_profile(db, ws, mods, zzz(), " Casual ")
        .await
        .unwrap();
    assert_eq!((saved.name.as_str(), saved.mods), ("Casual", 2));

    // Everything flipped, including Phoenix (renamed to DISABLED Phoenix).
    gmm_core::set_enabled(db, ws, mods, zzz(), red, false)
        .await
        .unwrap();
    gmm_core::set_enabled(db, ws, mods, zzz(), blue, true)
        .await
        .unwrap();
    gmm_core::set_unmanaged_enabled_at(db, ws, mods, zzz(), "Phoenix", false)
        .await
        .unwrap();
    assert!(f.mods_dir.join("DISABLED Phoenix").exists());

    let applied = gmm_core::apply_profile(db, ws, mods, zzz(), saved.id)
        .await
        .unwrap();
    assert_eq!(applied.switched_on.len(), 2, "{applied:?}");
    assert_eq!(applied.switched_off.len(), 1);
    assert!(applied.missing.is_empty());
    assert!(f.mods_dir.join("Phoenix").exists());
    assert!(db.installed_mod(red).await.unwrap().unwrap().enabled);
    assert!(!db.installed_mod(blue).await.unwrap().unwrap().enabled);

    // Sharing: GameBanana ids only, hand-installed mods left out.
    let code = gmm_core::export_profile(db, zzz(), saved.id).await.unwrap();
    assert!(
        code.contains("652373") && !code.contains("Phoenix"),
        "{code}"
    );
    let imported = gmm_core::import_profile(db, ws, mods, zzz(), &code, Some("Shared"))
        .await
        .unwrap();
    assert_eq!((imported.profile.mods, imported.missing.len()), (1, 0));

    let other = r#"{"gmm_profile":1,"game":"zzz","name":"Theirs","mods":[{"gb_mod_id":5,"gb_file_id":null,"variant":null,"name":"Not here"}]}"#;
    let theirs = gmm_core::import_profile(db, ws, mods, zzz(), other, None)
        .await
        .unwrap();
    assert_eq!(theirs.missing[0].name, "Not here");
    assert_eq!(theirs.profile.mods, 0);

    for hostile in [
        "not json",
        r#"{"gmm_profile":2,"game":"zzz","name":"x","mods":[]}"#,
        r#"{"gmm_profile":1,"game":"wuwa","name":"x","mods":[]}"#,
        r#"{"gmm_profile":1,"game":"../etc","name":"x","mods":[]}"#,
        r#"{"gmm_profile":1,"game":"zzz","name":"","mods":[]}"#,
    ] {
        assert!(
            gmm_core::import_profile(db, ws, mods, zzz(), hostile, None)
                .await
                .is_err(),
            "{hostile}"
        );
    }
    let huge = format!(
        r#"{{"gmm_profile":1,"game":"zzz","name":"x","mods":[],"pad":"{}"}}"#,
        "a".repeat(gmm_core::MAX_SHARE_CODE)
    );
    assert!(
        gmm_core::import_profile(db, ws, mods, zzz(), &huge, None)
            .await
            .is_err()
    );
    let names: Vec<String> = gmm_core::profiles(db, zzz())
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Casual", "Shared", "Theirs"]);
}

#[tokio::test]
async fn local_archives_go_through_the_same_pipeline() {
    let f = Fixture::new().await;
    let elsewhere = tempfile::tempdir().unwrap();
    let file = elsewhere.path().join("ellen_maid_v2.zip");
    fs::write(
        &file,
        zip_of(&[
            ("Ellen/ellen.ini", INI),
            ("Ellen/body.dds", b"dds"),
            ("Ellen/setup.exe", b"MZ"),
        ]),
    )
    .unwrap();

    let prepared = gmm_core::prepare_local(&f.ws, zzz(), &file).unwrap();
    assert_eq!(prepared.name, "ellen maid v2");
    let plan = gmm_core::local_plan(&f.db, zzz(), &prepared).await.unwrap();
    assert_eq!(plan.kind.as_deref(), Some("migoto"));
    assert_eq!(plan.versions.len(), 1);
    assert_eq!(plan.versions[0].folder, "ellen maid v2");
    assert_eq!(plan.versions[0].blocked, ["setup.exe"]);

    let installed = gmm_core::install_local(
        &f.db,
        &f.ws,
        f.mods,
        zzz(),
        &prepared,
        &InstallOptions {
            skip_blocked: true,
            ..InstallOptions::default()
        },
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(installed[0].skipped, ["setup.exe"]);
    assert!(f.gmm().join("ellen maid v2/ellen.ini").exists());
    assert!(!f.gmm().join("ellen maid v2/setup.exe").exists());
    let row = f.db.installed_mod(installed[0].id).await.unwrap().unwrap();
    assert_eq!((row.gb_mod_id, row.gb_file_id), (None, None));

    prepared.discard(&f.ws);
    assert!(file.exists(), "the user's file is untouched");
    let leftovers: Vec<_> = fs::read_dir(f.cache_dir.join("downloads"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // Only archives, only files.
    fs::write(elsewhere.path().join("notes.txt"), b"hi").unwrap();
    for bad in [
        elsewhere.path().join("notes.txt"),
        elsewhere.path().to_path_buf(),
    ] {
        assert!(matches!(
            gmm_core::prepare_local(&f.ws, zzz(), &bad),
            Err(CoreError::CantInstallLocal(_))
        ));
    }
}
