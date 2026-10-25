// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use gmm_catalog::HttpCache;
use gmm_db::{DB_FILE_NAME, Db, DbError, ModCategories, NewCachedDownload, NewInstalledMod};
use gmm_fsguard::{Access, FsError, Guard};

const HOUR: Duration = Duration::from_secs(3600);

fn new_mod(game: &str, folder: &str) -> NewInstalledMod {
    NewInstalledMod {
        game_id: game.to_owned(),
        folder_name: folder.to_owned(),
        name: folder.to_owned(),
        gb_mod_id: Some(652_373),
        gb_file_id: Some(1_400_001),
        version: Some("1.2".to_owned()),
        size_bytes: 1024,
        ..NewInstalledMod::default()
    }
}

#[tokio::test]
async fn file_database_is_created_migrated_and_reopened() {
    let dir = tempfile::tempdir().unwrap();
    let mut guard = Guard::new();
    let data = guard
        .register_root("data", dir.path(), Access::ReadWrite)
        .unwrap();
    let file = guard.path(data, DB_FILE_NAME).unwrap();

    let db = Db::open(&guard, &file).await.unwrap();
    assert_eq!(db.schema_version().await.unwrap(), 4);
    db.set_xxmi_root(Some(Path::new(r"D:\XXMI Launcher")))
        .await
        .unwrap();
    db.close().await;
    assert!(guard.exists(&file).unwrap());

    // Reopening runs no migration twice and keeps the data.
    let db = Db::open(&guard, &file).await.unwrap();
    assert_eq!(db.schema_version().await.unwrap(), 4);
    assert_eq!(
        db.xxmi_root().await.unwrap().as_deref(),
        Some(Path::new(r"D:\XXMI Launcher"))
    );
    db.close().await;
}

#[tokio::test]
async fn refuses_a_read_only_root() {
    let dir = tempfile::tempdir().unwrap();
    let mut guard = Guard::new();
    let root = guard
        .register_root("game", dir.path(), Access::ReadOnly)
        .unwrap();
    let file = guard.path(root, DB_FILE_NAME).unwrap();
    let err = Db::open(&guard, &file).await.unwrap_err();
    assert!(
        matches!(err, DbError::Fs(FsError::ReadOnlyRoot(_))),
        "{err:?}"
    );
    assert!(!guard.exists(&file).unwrap(), "nothing was created");
}

#[tokio::test]
async fn settings_round_trip_and_clear() {
    let db = Db::open_in_memory().await.unwrap();
    assert_eq!(db.confirmed_xxmi_exe().await.unwrap(), None);
    let exe =
        Path::new(r"C:\Users\u\AppData\Roaming\XXMI Launcher\Resources\Bin\XXMI Launcher.exe");
    db.set_confirmed_xxmi_exe(Some(exe)).await.unwrap();
    db.set_confirmed_xxmi_exe(Some(exe)).await.unwrap();
    assert_eq!(db.confirmed_xxmi_exe().await.unwrap().as_deref(), Some(exe));
    db.set_confirmed_xxmi_exe(None).await.unwrap();
    assert_eq!(db.confirmed_xxmi_exe().await.unwrap(), None);
    assert_eq!(db.xxmi_root().await.unwrap(), None, "keys are independent");
}

#[tokio::test]
async fn preferences_default_then_round_trip() {
    let db = Db::open_in_memory().await.unwrap();
    let d = db.preferences().await.unwrap();
    assert_eq!(d, gmm_db::Preferences::default());
    assert!(
        !d.show_sensitive && d.blur_sensitive && !d.onboarded,
        "safe defaults"
    );
    let chosen = gmm_db::Preferences {
        theme: gmm_db::Theme::System,
        show_sensitive: true,
        blur_sensitive: false,
        onboarded: true,
    };
    db.set_preferences(&chosen).await.unwrap();
    assert_eq!(db.preferences().await.unwrap(), chosen);
}

#[tokio::test]
async fn manual_game_folders() {
    let db = Db::open_in_memory().await.unwrap();
    let zzz = Path::new(r"E:\Games\ZenlessZoneZero Game");
    assert!(db.add_manual_game_folder("zzz", zzz).await.unwrap());
    assert!(
        !db.add_manual_game_folder("zzz", zzz).await.unwrap(),
        "duplicate"
    );
    assert!(
        db.add_manual_game_folder("wuwa", Path::new(r"E:\Games\Wuthering Waves"))
            .await
            .unwrap()
    );

    let folders = db.manual_game_folders().await.unwrap();
    assert_eq!(folders.len(), 2);
    assert!(
        folders
            .iter()
            .any(|f| f.game_id == "zzz" && f.folder == zzz)
    );

    assert!(
        !db.remove_manual_game_folder("wuwa", zzz).await.unwrap(),
        "wrong game"
    );
    let spelled = if cfg!(windows) {
        Path::new(r"e:\games\zenlesszonezero game")
    } else {
        zzz
    };
    assert!(db.remove_manual_game_folder("zzz", spelled).await.unwrap());
    assert_eq!(db.manual_game_folders().await.unwrap().len(), 1);
}

#[tokio::test]
async fn installed_mods_lifecycle() {
    let db = Db::open_in_memory().await.unwrap();
    let a = db
        .insert_installed_mod(&new_mod("zzz", "Ellen - Maid [gb-652373]"))
        .await
        .unwrap();
    let b = db
        .insert_installed_mod(&NewInstalledMod {
            variant: Some("Variant B".to_owned()),
            ..new_mod("zzz", "Ellen - Maid [gb-652373] (Variant B)")
        })
        .await
        .unwrap();
    db.insert_installed_mod(&NewInstalledMod {
        gb_mod_id: None,
        gb_file_id: None,
        ..new_mod("wuwa", "Local Mod")
    })
    .await
    .unwrap();

    // Same folder name in the same game is refused; in another game it's fine.
    assert!(matches!(
        db.insert_installed_mod(&new_mod("zzz", "Ellen - Maid [gb-652373]"))
            .await,
        Err(DbError::Sql(_))
    ));
    db.insert_installed_mod(&new_mod("wuwa", "Ellen - Maid [gb-652373]"))
        .await
        .unwrap();

    let m = db.installed_mod(a).await.unwrap().unwrap();
    assert!(m.enabled, "new mods start enabled");
    assert_eq!(m.gb_mod_id, Some(652_373));
    assert_eq!(m.size_bytes, 1024);
    assert!(m.installed_at > 0);

    assert_eq!(db.installed_mods(Some("zzz")).await.unwrap().len(), 2);
    assert_eq!(db.installed_mods(None).await.unwrap().len(), 4);
    assert_eq!(db.installed_mods_for_gb(652_373).await.unwrap().len(), 3);
    let by_folder = db
        .installed_mod_by_folder("zzz", "Ellen - Maid [gb-652373] (Variant B)")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_folder.id, b);
    assert_eq!(by_folder.variant.as_deref(), Some("Variant B"));

    assert!(db.set_mod_enabled(a, false).await.unwrap());
    assert!(!db.installed_mod(a).await.unwrap().unwrap().enabled);
    assert!(db.set_mod_size(a, 4096).await.unwrap());
    assert_eq!(db.installed_mod(a).await.unwrap().unwrap().size_bytes, 4096);

    assert!(db.delete_installed_mod(a).await.unwrap());
    assert!(!db.delete_installed_mod(a).await.unwrap());
    assert!(!db.set_mod_enabled(a, true).await.unwrap());
    assert_eq!(db.installed_mod(a).await.unwrap(), None);
}

#[tokio::test]
async fn categories_are_stored_with_the_mod() {
    let db = Db::open_in_memory().await.unwrap();
    let categories = ModCategories {
        root_category_id: Some(30_305),
        root_category: Some("Character Skins".to_owned()),
        category_id: Some(43_004),
        category: Some("Ellen".to_owned()),
    };
    let id = db
        .insert_installed_mod(&NewInstalledMod {
            categories: categories.clone(),
            ..new_mod("zzz", "Ellen - Maid [gb-652373]")
        })
        .await
        .unwrap();
    let plain = db
        .insert_installed_mod(&new_mod("zzz", "No categories"))
        .await
        .unwrap();
    assert_eq!(
        db.installed_mod(id).await.unwrap().unwrap().categories,
        categories
    );
    assert_eq!(
        db.installed_mod(plain).await.unwrap().unwrap().categories,
        ModCategories::default()
    );
}

#[tokio::test]
async fn last_played_per_game() {
    let db = Db::open_in_memory().await.unwrap();
    assert!(db.last_played().await.unwrap().is_empty());
    db.record_played("zzz").await.unwrap();
    db.record_played("wuwa").await.unwrap();
    db.record_played("zzz").await.unwrap();
    let played = db.last_played().await.unwrap();
    let games: Vec<&str> = played.iter().map(|(g, _)| g.as_str()).collect();
    assert_eq!(games, ["wuwa", "zzz"]);
    assert!(played.iter().all(|(_, t)| *t > 0));
}

#[tokio::test]
async fn download_cache_records() {
    let db = Db::open_in_memory().await.unwrap();
    let first = NewCachedDownload {
        sha256: "aa".repeat(32),
        gb_mod_id: Some(1),
        gb_file_id: Some(10),
        path_rel: "downloads/10-mod.zip".to_owned(),
        size_bytes: 1000,
    };
    db.record_download(&first).await.unwrap();
    db.record_download(&NewCachedDownload {
        sha256: "bb".repeat(32),
        gb_file_id: Some(11),
        path_rel: "downloads/11-mod.7z".to_owned(),
        size_bytes: 500,
        ..first.clone()
    })
    .await
    .unwrap();
    // Same bytes again: one row, refreshed.
    db.record_download(&first).await.unwrap();

    assert_eq!(db.cached_downloads().await.unwrap().len(), 2);
    assert_eq!(db.download_cache_bytes().await.unwrap(), 1500);
    let hit = db.cached_download_for_file(10).await.unwrap().unwrap();
    assert_eq!(hit.path_rel, "downloads/10-mod.zip");
    assert_eq!(db.cached_download_for_file(99).await.unwrap(), None);

    assert!(db.touch_download(&first.sha256).await.unwrap());
    assert!(db.remove_download(&first.sha256).await.unwrap());
    assert!(!db.touch_download(&first.sha256).await.unwrap());
    assert_eq!(db.cached_download(&first.sha256).await.unwrap(), None);
    assert_eq!(db.download_cache_bytes().await.unwrap(), 500);
}

#[tokio::test]
async fn http_cache_through_the_catalog_trait() {
    let db = Db::open_in_memory().await.unwrap();
    let cache: Arc<dyn HttpCache> = Arc::new(db.clone());
    let url = "https://gamebanana.com/apiv11/Mod/1/ProfilePage";

    assert_eq!(cache.get(url, HOUR).await, None);
    cache.put(url, b"{\"_idRow\":1}".to_vec()).await;
    assert_eq!(cache.get(url, HOUR).await, Some(b"{\"_idRow\":1}".to_vec()));
    cache.put(url, b"{\"_idRow\":2}".to_vec()).await;
    assert_eq!(cache.get(url, HOUR).await, Some(b"{\"_idRow\":2}".to_vec()));

    assert_eq!(db.http_cache_stats().await.unwrap(), (1, 12));
    assert_eq!(db.clear_http_cache().await.unwrap(), 1);
    assert_eq!(cache.get(url, HOUR).await, None);
}

#[tokio::test]
async fn profiles_save_replace_rename_delete() {
    let db = Db::open_in_memory().await.unwrap();
    let a = db.save_profile("zzz", "Casual", "[1]").await.unwrap();
    let again = db.save_profile("zzz", "Casual", "[2]").await.unwrap();
    assert_eq!(a, again, "same name replaces");
    assert_eq!(db.profile(a).await.unwrap().unwrap().entries_json, "[2]");
    let b = db.save_profile("zzz", "Battle", "[]").await.unwrap();
    db.save_profile("wuwa", "Casual", "[]").await.unwrap();
    let names: Vec<String> = db
        .profiles("zzz")
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Battle", "Casual"]);
    assert!(!db.rename_profile(b, "Casual").await.unwrap(), "name taken");
    assert!(db.rename_profile(b, "Combat").await.unwrap());
    assert!(db.delete_profile(a).await.unwrap());
    assert!(db.profile(a).await.unwrap().is_none());
}
