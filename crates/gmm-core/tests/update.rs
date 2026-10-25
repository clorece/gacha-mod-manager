// SPDX-License-Identifier: MIT OR Apache-2.0

//! One-click update and roll back (`gmm_core::change_file`) against a mock GameBanana with two
//! files of one mod, on a fake XXMI importer folder in a temp dir.

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::{GameBanana, NoCache};
use gmm_core::{
    CoreError, InstallOptions, InstallRequest, Remover, StopSignal, VariantChoice, Workspace,
    change_file, install, rollback_choices, set_enabled,
};
use gmm_db::Db;
use gmm_fsguard::{Guard, RootId, ScopedPath};
use gmm_net::{Allowlist, Client, NetConfig, RateLimit, Url};
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};
use zip::write::{SimpleFileOptions, ZipWriter};

const INI: &[u8] = b"[TextureOverrideBody]\nhash = 0123abcd\nib = ResourceBodyIB\n";
const MOD: u64 = 900_001;
const OLD: u64 = 1_400_001;
const NEW: u64 = 1_400_002;

fn zzz() -> &'static GameManifest {
    Registry::builtin().unwrap().get("zzz").unwrap()
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        w.start_file(*name, SimpleFileOptions::default()).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

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

#[derive(Default)]
struct FakeRemover(Mutex<Vec<String>>);

impl Remover for FakeRemover {
    fn remove(&self, guard: &Guard, path: &ScopedPath) -> Result<(), CoreError> {
        self.0.lock().unwrap().push(path.rel().to_string());
        guard.remove_dir_all(path)?;
        Ok(())
    }
}

struct Env {
    _tmp: TempDir,
    mods_dir: PathBuf,
    ws: Workspace,
    mods: RootId,
    db: Db,
    server: MockServer,
    gb: GameBanana,
    net: Client,
}

impl Env {
    /// GameBanana with two files of one mod: `old` (uploaded first) and `new`.
    async fn new(old: &[u8], new: &[u8]) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let mods_dir = tmp.path().join("XXMI").join("ZZMI").join("Mods");
        fs::create_dir_all(&mods_dir).unwrap();
        let mut ws = Workspace::open(&tmp.path().join("cache")).unwrap();
        let mods = ws.add_mods_root(&mods_dir).unwrap();
        let server = MockServer::start().await;
        let file = |id: u64, name: &str, data: &[u8], date: i64| {
            json!({
                "_idRow": id, "_sFile": name, "_nFilesize": data.len(), "_tsDateAdded": date,
                "_sDownloadUrl": format!("{}/dl/{id}", server.uri()), "_sAvResult": "clean",
            })
        };
        let files = json!({ "_aFiles": [
            file(NEW, "outfit_v2.zip", new, 1_790_100_000),
            file(OLD, "outfit_v1.zip", old, 1_790_000_000),
        ]});
        let profile = json!({
            "_idRow": MOD, "_sName": "Test Outfit A",
            "_aGame": { "_idRow": 19567 }, "_sVersion": "2.0",
        });
        for (p, body) in [
            (format!("/apiv11/Mod/{MOD}/ProfilePage"), profile),
            (format!("/apiv11/Mod/{MOD}/DownloadPage"), files),
        ] {
            Mock::given(path(p))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .mount(&server)
                .await;
        }
        for (id, data) in [(OLD, old), (NEW, new)] {
            Mock::given(path(format!("/dl/{id}")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(data.to_vec()))
                .mount(&server)
                .await;
        }
        let base = Url::parse(&format!("{}/apiv11/", server.uri())).unwrap();
        let gb = GameBanana::with_base_url(net(), Arc::new(NoCache), base);
        Self {
            _tmp: tmp,
            mods_dir,
            ws,
            mods,
            db: Db::open_in_memory().await.unwrap(),
            server,
            gb,
            net: net(),
        }
    }

    async fn install_old(&self, variants: VariantChoice) -> Vec<i64> {
        let report = install(
            &self.gb,
            &self.net,
            &self.db,
            &self.ws,
            self.mods,
            zzz(),
            &InstallRequest {
                mod_id: MOD,
                file_id: Some(OLD),
                options: InstallOptions {
                    variants,
                    ..InstallOptions::default()
                },
            },
            &StopSignal::new(),
            |_| {},
        )
        .await
        .unwrap();
        report.installed.iter().map(|f| f.id).collect()
    }

    async fn change(
        &self,
        file: Option<u64>,
        remover: &FakeRemover,
    ) -> Result<gmm_core::FileChangeReport, CoreError> {
        change_file(
            &self.gb,
            &self.net,
            &self.db,
            &self.ws,
            self.mods,
            zzz(),
            MOD,
            file,
            remover,
            &StopSignal::new(),
            |_| {},
        )
        .await
    }

    fn gmm(&self) -> PathBuf {
        self.mods_dir.join("GMM")
    }

    async fn downloads_of(&self, id: u64) -> usize {
        let wanted = format!("/dl/{id}");
        self.server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.path() == wanted)
            .count()
    }
}

#[tokio::test]
async fn update_keeps_the_folder_name_and_switch_then_rolls_back_from_the_cache() {
    let old = zip_of(&[("Outfit/outfit.ini", INI), ("Outfit/body.dds", b"v1")]);
    let new = zip_of(&[
        ("Outfit/outfit.ini", INI),
        ("Outfit/body.dds", b"v2 is here"),
    ]);
    let env = Env::new(&old, &new).await;
    let ids = env.install_old(VariantChoice::Only).await;
    let folder = "Test Outfit A [gb-900001]";
    // Switched off by the user before updating: it must stay off.
    set_enabled(&env.db, &env.ws, env.mods, zzz(), ids[0], false)
        .await
        .unwrap();

    let remover = FakeRemover::default();
    let report = env.change(None, &remover).await.unwrap();
    assert_eq!(report.file.id, NEW);
    assert_eq!(report.changed.len(), 1);
    assert_eq!(report.changed[0].id, ids[0], "same database row");
    assert!(!report.changed[0].enabled);
    let disabled = env.gmm().join(format!("DISABLED {folder}"));
    assert_eq!(fs::read(disabled.join("body.dds")).unwrap(), b"v2 is here");
    assert!(!env.gmm().join(folder).exists());
    let row = env.db.installed_mod(ids[0]).await.unwrap().unwrap();
    assert_eq!(
        (row.gb_file_id, row.folder_name.as_str(), row.enabled),
        (Some(NEW), folder, false)
    );
    let sidecar: serde_json::Value =
        serde_json::from_slice(&fs::read(disabled.join("gmm.json")).unwrap()).unwrap();
    assert_eq!(sidecar["gb_file_id"], NEW);
    assert_eq!(
        remover.0.lock().unwrap().len(),
        1,
        "previous folder removed"
    );

    // Roll back: the old archive is still cached, so nothing is downloaded again.
    assert_eq!(rollback_choices(&env.db, zzz(), MOD).await.unwrap(), [OLD]);
    let before = env.downloads_of(OLD).await;
    let back = env.change(Some(OLD), &remover).await.unwrap();
    assert_eq!(back.file.id, OLD);
    assert_eq!(env.downloads_of(OLD).await, before);
    assert_eq!(fs::read(disabled.join("body.dds")).unwrap(), b"v1");
    let gmm_entries: Vec<String> = fs::read_dir(env.gmm())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "DISABLED .staging")
        .collect();
    assert_eq!(gmm_entries, [format!("DISABLED {folder}")]);

    // Already on that file: nothing to do.
    let same = env.change(Some(OLD), &remover).await.unwrap();
    assert!(same.changed.is_empty());
    assert_eq!(same.kept, [folder]);
}

#[tokio::test]
async fn versions_are_matched_by_name_and_missing_ones_are_kept() {
    let old = zip_of(&[
        ("Red/red.ini", INI),
        ("Red/a.dds", b"red v1"),
        ("Blue/blue.ini", INI),
        ("Blue/a.dds", b"blue v1"),
    ]);
    // The new file dropped Blue.
    let new = zip_of(&[("Red/red.ini", INI), ("Red/a.dds", b"red v2")]);
    let env = Env::new(&old, &new).await;
    env.install_old(VariantChoice::All).await;

    let remover = FakeRemover::default();
    let report = env.change(None, &remover).await.unwrap();
    assert_eq!(report.changed.len(), 1);
    assert!(report.changed[0].folder_name.contains("(Red)"));
    assert_eq!(report.kept.len(), 1);
    assert!(report.kept[0].contains("(Blue)"));
    let red = env.gmm().join("Test Outfit A [gb-900001] (Red)");
    assert_eq!(fs::read(red.join("a.dds")).unwrap(), b"red v2");
    let blue = env.gmm().join("Test Outfit A [gb-900001] (Blue)");
    assert_eq!(fs::read(blue.join("a.dds")).unwrap(), b"blue v1");
}

#[tokio::test]
async fn a_file_without_a_mod_changes_nothing() {
    let old = zip_of(&[("Outfit/outfit.ini", INI)]);
    let new = zip_of(&[("readme.txt", b"moved to another page")]);
    let env = Env::new(&old, &new).await;
    let ids = env.install_old(VariantChoice::Only).await;
    let err = env.change(None, &FakeRemover::default()).await.unwrap_err();
    assert!(matches!(err, CoreError::NotAMod { .. }), "{err}");
    let row = env.db.installed_mod(ids[0]).await.unwrap().unwrap();
    assert_eq!(row.gb_file_id, Some(OLD));
    assert!(
        env.gmm()
            .join("Test Outfit A [gb-900001]/outfit.ini")
            .exists()
    );
}

#[tokio::test]
async fn not_installed_is_refused() {
    let env = Env::new(&zip_of(&[("a/a.ini", INI)]), &zip_of(&[("a/a.ini", INI)])).await;
    let err = env.change(None, &FakeRemover::default()).await.unwrap_err();
    assert!(matches!(err, CoreError::NotInstalled { .. }), "{err}");
}

#[tokio::test]
async fn a_hand_installed_folder_is_adopted_then_updates_like_any_other() {
    let old = zip_of(&[("Outfit/outfit.ini", INI)]);
    let new = zip_of(&[("Outfit/outfit.ini", INI), ("Outfit/body.dds", b"v2")]);
    let env = Env::new(&old, &new).await;
    Mock::given(path("/apiv11/Util/Search/Results"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "_aMetadata": { "_nRecordCount": 0, "_bIsComplete": true }, "_aRecords": [],
        })))
        .mount(&env.server)
        .await;
    let hand = env.mods_dir.join("Outfit by hand [gb-900001]");
    fs::create_dir_all(&hand).unwrap();
    fs::write(hand.join("outfit.ini"), INI).unwrap();
    fs::write(env.mods_dir.join("loose.ini"), INI).unwrap();

    let found = gmm_core::adopt_candidates(
        &env.gb,
        &env.db,
        &env.ws,
        env.mods,
        zzz(),
        "Outfit by hand [gb-900001]",
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].summary.id, MOD);
    assert_eq!(found[0].reason, gmm_core::MatchReason::IdInName);

    let adopted = gmm_core::adopt(
        &env.gb,
        &env.db,
        &env.ws,
        env.mods,
        zzz(),
        "Outfit by hand [gb-900001]",
        MOD,
        Some(OLD),
    )
    .await
    .unwrap();
    assert!(adopted.moved);
    assert!(!hand.exists());
    let inside = env.gmm().join("Outfit by hand [gb-900001]");
    assert!(inside.join("outfit.ini").exists() && inside.join("gmm.json").exists());
    let row = env.db.installed_mod(adopted.id).await.unwrap().unwrap();
    assert_eq!((row.gb_mod_id, row.gb_file_id), (Some(MOD), Some(OLD)));

    // Now it's GMM's: an update swaps its files in under the same name.
    let report = env.change(None, &FakeRemover::default()).await.unwrap();
    assert_eq!(report.changed.len(), 1);
    assert_eq!(fs::read(inside.join("body.dds")).unwrap(), b"v2");

    // Loose ini files and folders GMM already has are refused.
    let loose = gmm_core::adopt(
        &env.gb,
        &env.db,
        &env.ws,
        env.mods,
        zzz(),
        "loose.ini",
        MOD,
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(loose, CoreError::CantAdopt(_)), "{loose}");
}

#[tokio::test]
async fn a_lost_gmm_folder_is_found_by_its_sidecar() {
    let env = Env::new(&zip_of(&[("a/a.ini", INI)]), &zip_of(&[("a/a.ini", INI)])).await;
    Mock::given(path("/apiv11/Util/Search/Results"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&env.server)
        .await;
    let lost = env.gmm().join("DISABLED_Renamed");
    fs::create_dir_all(&lost).unwrap();
    fs::write(lost.join("a.ini"), INI).unwrap();
    fs::write(
        lost.join("gmm.json"),
        br#"{"gb_mod_id": 900001, "gb_file_id": 1400002, "anything": "else"}"#,
    )
    .unwrap();
    let found = gmm_core::adopt_candidates(
        &env.gb,
        &env.db,
        &env.ws,
        env.mods,
        zzz(),
        "GMM/DISABLED_Renamed",
    )
    .await
    .unwrap();
    assert_eq!(found[0].reason, gmm_core::MatchReason::Sidecar);
    assert_eq!(found[0].file_id, Some(NEW));
    let adopted = gmm_core::adopt(
        &env.gb,
        &env.db,
        &env.ws,
        env.mods,
        zzz(),
        "GMM/DISABLED_Renamed",
        MOD,
        found[0].file_id,
    )
    .await
    .unwrap();
    assert_eq!(adopted.folder_name, "Renamed");
    assert!(env.gmm().join("DISABLED Renamed").exists(), "stays off");
    assert!(
        !env.db
            .installed_mod(adopted.id)
            .await
            .unwrap()
            .unwrap()
            .enabled
    );
}
