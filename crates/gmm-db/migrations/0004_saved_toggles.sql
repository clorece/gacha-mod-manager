-- SPDX-License-Identifier: MIT OR Apache-2.0
-- 3DMigoto's saved in-game toggles of switched-off mods (R3 amendment, 2026-10-25).

-- 3DMigoto keeps a mod's toggle choices in `<IMP>/d3dx_user.ini`, keyed by the mod's path,
-- and drops them once the mod stops loading. GMM copies a mod's lines here when it switches
-- the mod off and writes them back when it switches the mod on. `mod_path` is the mod's
-- path under Mods/ in its switched-on form, lowercased with `/` separators
-- (`gmm/ellen - maid [gb-1]`); `lines` holds the `key = value` lines, one per line.
-- `pending` is 1 while a write-back waits for the game to close.
CREATE TABLE saved_toggles (
    game_id  TEXT NOT NULL,
    mod_path TEXT NOT NULL,
    lines    TEXT NOT NULL,
    pending  INTEGER NOT NULL DEFAULT 0,
    saved_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, mod_path)
) STRICT;
