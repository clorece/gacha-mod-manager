// SPDX-License-Identifier: MIT OR Apache-2.0
// Generated from the Rust types by apps/desktop/src-tauri/src/bindings.rs. Don't edit;
// run `GMM_UPDATE_BINDINGS=1 cargo test -p gmm-desktop bindings` instead.

export type CmdError = { "kind": "choose_variant", message: string, variants: Array<string>, } | { "kind": "blocked", message: string, files: Array<string>, } | { "kind": "conflicts", message: string, conflicts: Array<Conflict>, } | { "kind": "not_ready", message: string, } | { "kind": "other", message: string, };

export type GameInfo = { id: string, name: string, short_name: string, importer: string, gamebanana_id: number, 
/**
 * When GMM's Launch button last started it (Unix seconds).
 */
last_played: number | null, 
/**
 * The Store's Category filter, from the game's manifest (GameBanana's order).
 */
categories: Array<GameCategory>, };

export type GameCategory = { id: number, name: string, 
/**
 * Its subcategories are characters (the Store's Characters filter).
 */
characters: boolean, };

export type CatalogQuery = { game: string, 
/**
 * 1-based.
 */
page: number, per_page: number, sort: Sort, categories: Array<number>, text: string | null, show_nsfw: boolean, };

export type CatalogPage = { page: Page<ModSummary>, hidden: number, };

export type ModRow = { id: number, name: string, 
/**
 * The folder's name on disk right now (with `DISABLED ` when off).
 */
folder: string, enabled: boolean, disk: DiskState, variant: string | null, gb_mod_id: number | null, gb_file_id: number | null, 
/**
 * Folders with the same group are versions of one mod (same GameBanana mod and file).
 */
group: string | null, version: string | null, size_bytes: number, installed_at: number, 
/**
 * GameBanana's root category at install, e.g. "Character Skins".
 */
root_category: string | null, root_category_id: number | null, 
/**
 * The subcategory at install: for skins, the character.
 */
category: string | null, category_id: number | null, 
/**
 * GameBanana flagged it at install (false when unknown: earlier installs).
 */
sensitive: boolean, 
/**
 * The stored preview image as a `gmmimg` address, when there is one.
 */
preview: string | null, };

export type LibraryView = { game: string, 
/**
 * Why the Mods folder couldn't be read (XXMI or the importer missing), if it couldn't.
 */
problem: string | null, managed: Array<ModRow>, unmanaged: Array<UnmanagedMod>, };

export type ModKey = { "type": "managed", id: number, } | { "type": "unmanaged", path: string, };

export type VariantPick = { "type": "only" } | { "type": "named", names: Array<string>, } | { "type": "all" };

export type InstallArgs = { mod_id: number, 
/**
 * Defaults to the newest file GameBanana scanned as clean.
 */
file_id: number | null, variants: VariantPick, 
/**
 * Install without the blocked files (after the user saw them listed).
 */
skip_blocked: boolean, 
/**
 * Install switched off ("Switch it on after installing" unticked).
 */
start_disabled: boolean, };

export type InstallProgress = { "type": "using_cache" } | { "type": "downloading", size: number, } | { "type": "progress", downloaded: number, total: number | null, } | { "type": "extracting" } | { "type": "installing", folder: string, };

export type InstallResult = { report: InstallReport, 
/**
 * Setup problems that don't stop installing but stop mods from loading.
 */
setup_warnings: Array<string>, 
/**
 * Mods already on that the new ones clash with (both stay on).
 */
conflicts: Array<Conflict>, };

export type Toggled = { key: ModKey, enabled: boolean, 
/**
 * False when the mod already was in that state.
 */
changed: boolean, };

export type UninstallItem = { key: ModKey, name: string, 
/**
 * Full path, for the user to check.
 */
folder: string, 
/**
 * Counted for hand-installed mods only.
 */
files: number | null, bytes: number, 
/**
 * The size is at least this (the walk stopped early).
 */
truncated: boolean, 
/**
 * Not installed by GMM: the user's own files.
 */
hand_installed: boolean, };

export type Uninstalled = { key: ModKey, 
/**
 * False when the folder was already gone and only GMM's record was removed.
 */
removed: boolean, };

export type GameStorageView = { game: string, problem: string | null, storage: GameStorage | null, };

export type StorageView = { games: Array<GameStorageView>, cache: CacheStorage, };

export type LaunchInfo = { exe: string, args: Array<string>, 
/**
 * Whether the user already confirmed this exact exe.
 */
confirmed: boolean, };

export type Launched = { pid: number | null, };

export type FolderRow = { game: string, folder: string, };

export type Settings = { database: string, 
/**
 * The XXMI folder the user picked, or `None` when it's found automatically.
 */
xxmi_root: string | null, confirmed_xxmi_exe: string | null, manual_folders: Array<FolderRow>, http_cache_entries: number, http_cache_bytes: number, download_cache_entries: number, download_cache_bytes: number, };

export type StepState = "working" | "done" | "skipped" | "failed";

export type StartupStep = { 
/**
 * 0 is the GMM app, then one per game, then the installed mods.
 */
step: number, label: string, state: StepState, detail: string, };

export type StartupReport = { 
/**
 * GameBanana mods compared with their pages.
 */
checked: number, updates: Array<ModUpdateInfo>, 
/**
 * Mods or games that couldn't be checked.
 */
notes: Array<string>, };

export type CardStats = { mod_id: number, stats: ModStats, };

export type NetworkStatus = { 
/**
 * Seconds until GMM asks GameBanana again, while it holds off after a "slow down".
 */
cooldown_secs: number | null, 
/**
 * Saved answers stood in for fresh ones in the last minute and a half.
 */
showing_saved: boolean, };

export type RollbackFile = { file_id: number, name: string, 
/**
 * Upload date (Unix seconds), when GameBanana still lists the file.
 */
date_added: number | null, version: string | null, };

export type ThemeChoice = "dark" | "light" | "system";

export type Preferences = { theme: ThemeChoice, 
/**
 * Show mods GameBanana marks as sensitive (NSFW). Off by default.
 */
show_sensitive: boolean, 
/**
 * Blur sensitive previews until hovered, while they are shown. On by default.
 */
blur_sensitive: boolean, 
/**
 * The first-run preferences screen was answered.
 */
onboarded: boolean, };

export type GameStatus = { game_id: string, game_name: string, state: SetupState, 
/**
 * One sentence for the user.
 */
summary: string, 
/**
 * What to do next, in order. Empty when ready.
 */
steps: Array<string>, 
/**
 * Extra information that doesn't block anything.
 */
notes: Array<string>, detected_installs: Array<GameInstall>, xxmi_root: string | null, xxmi_version: string | null, importer: string, importer_info: ImporterInfo | null, 
/**
 * The game folder XXMI is configured with.
 */
xxmi_game_folder: string | null, 
/**
 * Which detected install that folder is, if any.
 */
xxmi_game_folder_source: InstallSource | null, 
/**
 * XXMI's "Game Launch" setting, raw (`DIRECT`, `STEAM`, ...).
 */
game_launch: string | null, launch_count: number | null, 
/**
 * Executable and arguments GMM would run to start the game with mods (only when ready).
 */
launch_command: Array<string> | null, };

export type SetupState = "xxmi_not_found" | "importer_missing" | "importer_broken" | "game_path_not_set" | "path_mismatch" | "launch_mode_mismatch" | "ready";

export type ImporterInfo = { folder_exists: boolean, 
/**
 * Critical files that are missing (relative to the importer folder).
 */
missing_files: Array<string>, 
/**
 * `target = ...` from `d3dx.ini`.
 */
d3dx_target: string | null, 
/**
 * Whether `d3dx.ini` loads mods from `Mods/` (`include_recursive = Mods`).
 */
d3dx_loads_mods: boolean, 
/**
 * `global $version` from `Core/<IMP>/main.ini`.
 */
importer_version: string | null, 
/**
 * File version of the XXMI DLL (`d3d11.dll`).
 */
dll_version: string | null, mods: ModsSummary, };

export type ModsSummary = { 
/**
 * Top-level folders in `Mods/` (each is usually one mod).
 */
enabled: number, 
/**
 * Top-level folders whose name starts with `DISABLED`.
 */
disabled: number, total_bytes: number, 
/**
 * True when the walk stopped early.
 */
truncated: boolean, };

export type GameInstall = { game_id: string, source: InstallSource, 
/**
 * Canonical path of the folder holding the game exe.
 */
folder: string, steam_appid: number | null, };

export type InstallSource = "steam" | "ho_yo_play" | "epic" | "manual";

export type Page<T> = { items: Array<T>, 
/**
 * Total matching records across all pages.
 */
total: number, page: number, per_page: number, is_last: boolean, };

export type Sort = "default" | "newest" | "recently_updated" | "most_downloaded" | "most_liked" | "most_viewed";

export type Visibility = "show" | "warn" | "hide" | { "other": string };

export type Member = { id: number, name: string, avatar_url: string | null, profile_url: string | null, };

export type CategoryRef = { 
/**
 * Absent when GameBanana only gives a name.
 */
id: number | null, name: string, };

export type ImageSize = { width: number, height: number, };

export type ImageVariant = { url: string, size: ImageSize, };

export type Image = { 
/**
 * Full-size original.
 */
url: string, 
/**
 * Scaled copies, smallest first.
 */
variants: Array<ImageVariant>, };

export type ModStats = { likes: number, views: number, posts: number, 
/**
 * Only on the mod page, not in listings.
 */
downloads: number | null, };

export type ModSummary = { id: number, 
/**
 * GameBanana game id (`_aGame`), when the response includes it (mod pages do;
 * observed 2026-10-23).
 */
game_id: number | null, name: string, submitter: Member, root_category: CategoryRef | null, 
/**
 * For skins this is the character.
 */
category: CategoryRef | null, version: string | null, date_added: number, date_modified: number, date_updated: number | null, images: Array<Image>, stats: ModStats, featured: boolean, obsolete: boolean, visibility: Visibility, has_content_ratings: boolean, profile_url: string, };

export type ModFile = { id: number, name: string, size: number, date_added: number, download_count: number, download_url: string, 
/**
 * Often empty on GameBanana.
 */
md5: string | null, version: string | null, description: string | null, 
/**
 * GameBanana's preliminary analysis, e.g. "ok".
 */
analysis_result: string | null, 
/**
 * GameBanana's antivirus result, e.g. "clean".
 */
av_result: string | null, archived: boolean, };

export type ModFiles = { files: Array<ModFile>, archived_files: Array<ModFile>, };

export type LicenseChecklist = { yes: Array<string>, ask: Array<string>, no: Array<string>, };

export type Credit = { name: string, role: string | null, member_id: number | null, };

export type CreditGroup = { name: string, authors: Array<Credit>, };

export type ModProfile = { summary: ModSummary, 
/**
 * Untrusted HTML; sanitize before display.
 */
description_html: string, 
/**
 * Untrusted HTML; sanitize before display.
 */
license_html: string, license: LicenseChecklist, credits: Array<CreditGroup>, files: ModFiles, updates_count: number, is_trashed: boolean, is_withheld: boolean, };

export type ChangeEntry = { text: string, 
/**
 * e.g. "Addition", "Bugfix".
 */
category: string, };

export type ModUpdate = { id: number, title: string, version: string | null, date_added: number, changelog: Array<ChangeEntry>, 
/**
 * Untrusted HTML; sanitize before display.
 */
text_html: string, file_ids: Array<number>, };

export type Category = { id: number, name: string, item_count: number, subcategory_count: number, icon_url: string | null, obsolete: boolean, };

export type TopPeriod = "today" | "week" | "month" | "three_months" | "six_months" | "year" | "all_time";

export type DiskState = "enabled" | "disabled" | "missing" | "ambiguous";

export type UnmanagedMod = { name: string, 
/**
 * Relative to `Mods/`, with `/`: `Phoenix`, or `GMM/Something` for a folder inside GMM's
 * own folder that the database doesn't know.
 */
path: string, enabled: boolean, is_dir: boolean, };

export type ModRef = { "type": "managed", id: number, folder: string, } | { "type": "unmanaged", path: string, };

export type Conflict = { a: ModRef, b: ModRef, 
/**
 * The hashes both replace, sorted.
 */
hashes: Array<string>, };

export type ConflictReport = { game: string, 
/**
 * Enabled mods whose ini files were read.
 */
mods_checked: number, conflicts: Array<Conflict>, 
/**
 * Mods skipped as shared libraries (by namespace).
 */
libraries: Array<ModRef>, 
/**
 * Problems that make the report less complete (unreadable files, very large mods).
 */
notes: Array<string>, };

export type Size = { bytes: number, files: number, 
/**
 * The walk stopped early; the real size is larger.
 */
truncated: boolean, };

export type ModUsage = { mod_ref: ModRef, enabled: boolean, size: Size, };

export type GameStorage = { game: string, 
/**
 * Largest first.
 */
managed: Array<ModUsage>, unmanaged: Array<ModUsage>, managed_total: Size, unmanaged_total: Size, 
/**
 * Loose files in `Mods/` and leftovers in GMM's staging folder.
 */
other: Size, 
/**
 * Free space on the drive that holds `Mods/`.
 */
free_bytes: number | null, };

export type CacheStorage = { 
/**
 * Archives in `downloads/` (including half-finished downloads).
 */
downloads: Size, 
/**
 * Leftover extraction folders (normally empty).
 */
extract: Size, 
/**
 * Preview images in `images/` (capped, oldest go first).
 */
images: Size, 
/**
 * GameBanana responses kept in the database.
 */
http_entries: number, http_bytes: number, free_bytes: number | null, };

export type ClearedCache = { archives: number, bytes: number, };

export type InstalledFolder = { 
/**
 * Database id.
 */
id: number, folder_name: string, variant: string | null, files: number, bytes: number, 
/**
 * Blocked files that were left out (only with [`InstallOptions::skip_blocked`]).
 */
skipped: Array<string>, 
/**
 * Scan warnings for this folder, as `path: reason`.
 */
warnings: Array<string>, };

export type InstallReport = { game: string, mod_id: number, mod_name: string, file_id: number, file_name: string, archive_sha256: string, from_cache: boolean, installed: Array<InstalledFolder>, };

export type ToggleOutcome = { id: number, folder_name: string, enabled: boolean, 
/**
 * False when the mod already was in that state.
 */
changed: boolean, };

export type InstallPlan = { game: string, mod_id: number, mod_name: string, 
/**
 * The file that was fetched and analysed.
 */
file: ModFile, 
/**
 * The mod's other current files, newest first (for "Choose a file").
 */
other_files: Array<ModFile>, archive_bytes: number, from_cache: boolean, 
/**
 * The mod kind from the game's manifest, or `None` when nothing in the archive looks
 * like a mod for this game (installing would fail).
 */
kind: string | null, versions: Array<VersionPlan>, 
/**
 * Versions that replace the same things; only one of each pair should be on.
 */
version_conflicts: Array<VersionConflict>, 
/**
 * Where it installs, relative to XXMI: `ZZMI\Mods\GMM`.
 */
install_to: string, };

export type VersionPlan = { 
/**
 * Empty for an archive with one mod; otherwise what `install` takes as a version name.
 */
name: string, 
/**
 * The folder it gets under `Mods/GMM/`.
 */
folder: string, files: number, bytes: number, 
/**
 * The installed copy's id, when this version is already installed.
 */
installed: number | null, 
/**
 * Files that are never installed (executables, scripts), relative to the version.
 */
blocked: Array<string>, 
/**
 * Scan warnings, as `path: reason`.
 */
warnings: Array<string>, };

export type VersionConflict = { 
/**
 * Positions in [`Analysis::variants`].
 */
a: number, b: number, 
/**
 * How many hashes both replace.
 */
hashes: number, };

export type Job = { id: number, action: JobAction, game: string, mod_id: number, mod_name: string, author: string | null, file_name: string | null, file_size: number | null, 
/**
 * The mod's preview, as the caller wants it shown (the app passes a `gmmimg` URL).
 */
image: string | null, 
/**
 * How many versions were chosen (0 when the archive holds one mod).
 */
versions: number, state: JobState, 
/**
 * Unix seconds.
 */
queued_at: number, finished_at: number | null, };

export type JobState = { "type": "queued" } | { "type": "downloading", downloaded: number, total: number | null, } | { "type": "extracting" } | { "type": "installing", folder: string, } | { "type": "waiting", until: number, } | { "type": "paused" } | { "type": "done", installed: Array<InstalledFolder>, } | { "type": "failed", message: string, } | { "type": "cancelled" };

export type ModUpdateInfo = { game: string, gb_mod_id: number, name: string, 
/**
 * The installed folders (versions) of this mod.
 */
installed_ids: Array<number>, installed_file_id: number | null, 
/**
 * Upload date of the installed file (Unix seconds).
 */
installed_file_date: number | null, 
/**
 * The newest file GameBanana scanned as clean.
 */
newest: ModFile, };

export type FeaturedMod = { 
/**
 * From the mod page, so the stats include downloads.
 */
summary: ModSummary, 
/**
 * The trending window it was picked from; `None` for a backfilled mod.
 */
period: TopPeriod | null, };

export type JobAction = "install" | "change_file";

export type ModKeyBinding = { 
/**
 * The `.ini` it's in, relative to the mod's folder (`/` separators).
 */
ini: string, 
/**
 * The section name without `Key` (`[KeySwap]` -> `Swap`), else as written.
 */
name: string, 
/**
 * The keys that trigger it, as written (`VK_LEFT`, `no_modifiers ctrl 1`).
 */
keys: Array<string>, 
/**
 * Keys that step backwards through a cycle.
 */
back: Array<string>, 
/**
 * `cycle`, `toggle`, `hold` or `activate`, when the section says.
 */
kind: string | null, };

export type KeyBindings = { bindings: Array<ModKeyBinding>, 
/**
 * Files that couldn't be read or were too large, as text.
 */
notes: Array<string>, };

export type ProfileInfo = { id: number, game: string, name: string, 
/**
 * How many mods it switches on.
 */
mods: number, updated_at: number, };

export type ProfileApplied = { switched_on: Array<string>, switched_off: Array<string>, 
/**
 * Mods in the profile that aren't in the Mods folder any more.
 */
missing: Array<string>, };

export type SharedMod = { gb_mod_id: number, gb_file_id: number | null, variant: string | null, name: string, };

export type ProfileImported = { profile: ProfileInfo, 
/**
 * Shared mods this GMM doesn't have; install them, then import again.
 */
missing: Array<SharedMod>, };

export type MatchReason = "sidecar" | "id_in_name" | "search";

export type AdoptCandidate = { summary: ModSummary, reason: MatchReason, 
/**
 * The file the folder's `gmm.json` names, for a [`MatchReason::Sidecar`] match.
 */
file_id: number | null, };

export type Adopted = { id: number, folder_name: string, 
/**
 * It was moved into `Mods/GMM/` (its in-game toggle state starts over).
 */
moved: boolean, };

export type FileChangeReport = { game: string, gb_mod_id: number, mod_name: string, 
/**
 * The file the mod is on now.
 */
file: ModFile, changed: Array<ChangedFolder>, 
/**
 * Folders left as they were: their version isn't in the new file, or they already
 * were on it.
 */
kept: Array<string>, 
/**
 * Previous folders that couldn't be deleted (left in the staging
 * folder, which GMM clears after an hour).
 */
not_removed: Array<string>, };

export type ChangedFolder = { id: number, folder_name: string, enabled: boolean, files: number, bytes: number, 
/**
 * Blocked files in the new file that were left out.
 */
skipped: Array<string>, };

export type LocalPlan = { game: string, name: string, file_name: string, archive_bytes: number, 
/**
 * The mod kind, or `None` when nothing in the archive is a mod for this game.
 */
kind: string | null, versions: Array<VersionPlan>, version_conflicts: Array<VersionConflict>, install_to: string, };

export type LogEntry = { 
/**
 * UTC, like `2026-10-25T21:03:11Z`.
 */
time: string, 
/**
 * Where it happened: `app`, `install`, `ui`, `panic`, `cli`.
 */
source: string, message: string, };

export type BugReport = { title: string, body: string, 
/**
 * The address that opens the pre-filled issue.
 */
url: string, 
/**
 * The description was cut to fit the address.
 */
shortened: boolean, };
