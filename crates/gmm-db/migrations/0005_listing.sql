-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Phase 5: what the library cards show without asking GameBanana (the third checkpoint).

-- Whether GameBanana flagged the mod as sensitive when it was installed (visibility or
-- content ratings), for the Library's Show sensitive filter; NULL when unknown (mods
-- installed before this migration, hand-installed ones).
ALTER TABLE installed_mods ADD COLUMN sensitive INTEGER;

-- The address of the mod's first preview image on GameBanana's image host. The app hands
-- the webview a `gmmimg` key for it, never the address (R5).
ALTER TABLE installed_mods ADD COLUMN preview_url TEXT;
