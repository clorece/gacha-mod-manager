-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Phase 5: what the library cards and the Dashboard show without asking GameBanana.

-- The mod's GameBanana categories when it was installed: the root category (e.g. "Character
-- Skins") and the subcategory (for skins, the character). Ids feed the store's filters;
-- names are shown. Mods installed before this migration, and hand-installed ones, have none.
ALTER TABLE installed_mods ADD COLUMN root_category_id INTEGER;
ALTER TABLE installed_mods ADD COLUMN root_category TEXT;
ALTER TABLE installed_mods ADD COLUMN category_id INTEGER;
ALTER TABLE installed_mods ADD COLUMN category TEXT;

-- When GMM's Launch button last started each game.
CREATE TABLE game_activity (
    game_id     TEXT PRIMARY KEY NOT NULL,
    last_played INTEGER NOT NULL
) STRICT;
