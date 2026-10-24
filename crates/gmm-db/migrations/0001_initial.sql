-- SPDX-License-Identifier: MIT OR Apache-2.0
-- First schema (Phase 2). See "Data model" in docs/02-architecture.md.
-- Times are Unix seconds (UTC). Paths are stored as text exactly as the user gave them.

-- Free-form app settings (the confirmed XXMI exe, an XXMI folder the user picked, ...).
-- Typed accessors live in src/settings.rs; nothing else reads this table directly.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

-- Game folders the user named because detection couldn't find them. Detection still
-- validates every one against the game's folder markers before it counts.
CREATE TABLE manual_game_folders (
    game_id  TEXT NOT NULL,
    folder   TEXT NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, folder)
) STRICT;

-- Mods GMM installed under <XXMI>/<IMP>/Mods/GMM/. Unmanaged mods are not stored: they're
-- listed from disk each time. `folder_name` never includes the `DISABLED ` prefix; the
-- folder on disk is `folder_name` when enabled and `DISABLED folder_name` when not.
CREATE TABLE installed_mods (
    id               INTEGER PRIMARY KEY,
    game_id          TEXT NOT NULL,
    folder_name      TEXT NOT NULL,
    name             TEXT NOT NULL,
    gb_mod_id        INTEGER,
    gb_file_id       INTEGER,
    variant          TEXT,
    version          TEXT,
    file_date        INTEGER,
    enabled          INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    archive_sha256   TEXT,
    size_bytes       INTEGER NOT NULL DEFAULT 0,
    installed_at     INTEGER NOT NULL,
    scan_report_json TEXT,
    UNIQUE (game_id, folder_name)
) STRICT;

CREATE INDEX installed_mods_gb ON installed_mods (gb_mod_id);

-- Original archives kept for reinstall and rollback, under the cache root's `downloads/`.
CREATE TABLE download_cache (
    sha256       TEXT PRIMARY KEY NOT NULL,
    gb_mod_id    INTEGER,
    gb_file_id   INTEGER,
    path_rel     TEXT NOT NULL UNIQUE,
    size_bytes   INTEGER NOT NULL,
    added_at     INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL
) STRICT;

CREATE INDEX download_cache_file ON download_cache (gb_file_id);

-- GameBanana API responses keyed by URL. Freshness is decided by the reader (each endpoint
-- has its own max age); old rows are pruned when the database opens.
CREATE TABLE http_cache (
    url        TEXT PRIMARY KEY NOT NULL,
    body       BLOB NOT NULL,
    fetched_at INTEGER NOT NULL
) STRICT;
