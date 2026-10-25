-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Phase 6: profiles, named sets of switched-on mods per game.

-- `entries_json` lists the mods that are on in the profile: GMM's by folder name (stable,
-- unique per game; database ids can be reused) with their GameBanana ids for sharing, and
-- hand-installed ones by their path under Mods/. Written and read only by gmm-core.
CREATE TABLE profiles (
    id           INTEGER PRIMARY KEY,
    game_id      TEXT NOT NULL,
    name         TEXT NOT NULL,
    entries_json TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    UNIQUE (game_id, name)
) STRICT;
