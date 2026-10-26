// SPDX-License-Identifier: MIT OR Apache-2.0
// Synthetic command results for the tests and the contrast audit: two games, a library with a
// version group, a sensitive mod and hand-installed folders, a Store page, a mod page. Shaped
// like what the Rust commands return (typed against bindings.ts, so a changed Rust type breaks
// the build here), with made-up names and paths: no real user data, no GameBanana text.

import type {
  CatalogPage,
  Category,
  FeaturedMod,
  GameInfo,
  GameStatus,
  Image,
  InstallPlan,
  Job,
  KeyBindings,
  LibraryView,
  ModFile,
  ModProfile,
  ModRow,
  ModSummary,
  ModUpdate,
  ModUpdateInfo,
  Page,
  Preferences,
  ProfileInfo,
  Settings,
  StorageView,
  UninstallItem,
} from "../api";

const DAY = 86_400;
/** A fixed "now" (2026-10-25 12:00 UTC), so dates in snapshots never move. */
export const NOW = 1_792_929_600;

/** A flat color image as a data address, so previews render in the audit without a network. */
export function art(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="10"><rect width="16" height="10" fill="${color}"/></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const image = (color: string): Image => ({ url: art(color), variants: [] });

export const games: GameInfo[] = [
  {
    id: "zzz",
    name: "Zenless Zone Zero",
    short_name: "ZZZ",
    importer: "ZZMI",
    gamebanana_id: 19567,
    last_played: NOW - 2 * DAY,
    categories: [
      { id: 30305, name: "Character Skins", characters: true },
      { id: 30306, name: "UI", characters: false },
    ],
  },
  {
    id: "wuwa",
    name: "Wuthering Waves",
    short_name: "WuWa",
    importer: "WWMI",
    gamebanana_id: 20357,
    last_played: null,
    categories: [{ id: 29496, name: "Skins", characters: true }],
  },
];

const importerInfo = (enabled: number, disabled: number) => ({
  folder_exists: true,
  missing_files: [],
  d3dx_target: "ZenlessZoneZero.exe",
  d3dx_loads_mods: true,
  importer_version: "1.50",
  dll_version: "1.2.2.0",
  mods: { enabled, disabled, total_bytes: 120_000_000, truncated: false },
});

export const statuses: GameStatus[] = [
  {
    game_id: "zzz",
    game_name: "Zenless Zone Zero",
    state: "ready",
    summary: "Ready: start Zenless Zone Zero through XXMI and mods load.",
    steps: [],
    notes: [],
    detected_installs: [
      { game_id: "zzz", source: "ho_yo_play", folder: "C:\\Games\\ZenlessZoneZero Game", steam_appid: null },
    ],
    xxmi_root: "C:\\Users\\player\\AppData\\Roaming\\XXMI Launcher",
    xxmi_version: "2.4.9",
    importer: "ZZMI",
    importer_info: importerInfo(3, 2),
    xxmi_game_folder: "C:\\Games\\ZenlessZoneZero Game",
    xxmi_game_folder_source: "ho_yo_play",
    game_launch: "DIRECT",
    launch_count: 7,
    launch_command: ["XXMI Launcher.exe", "--nogui", "--xxmi", "ZZMI"],
  },
  {
    game_id: "wuwa",
    game_name: "Wuthering Waves",
    state: "path_mismatch",
    summary: "XXMI's game folder for Wuthering Waves doesn't contain the game: D:\\Old",
    steps: [
      'Open XXMI Launcher, select Wuthering Waves, open the settings and the "General" tab.',
      'Set "Game Folder" to: C:\\Games\\Wuthering Waves',
    ],
    notes: [],
    detected_installs: [
      { game_id: "wuwa", source: "steam", folder: "C:\\Games\\Wuthering Waves", steam_appid: 3513350 },
    ],
    xxmi_root: "C:\\Users\\player\\AppData\\Roaming\\XXMI Launcher",
    xxmi_version: "2.4.9",
    importer: "WWMI",
    importer_info: importerInfo(1, 0),
    xxmi_game_folder: "D:\\Old",
    xxmi_game_folder_source: null,
    game_launch: "DIRECT",
    launch_count: 3,
    launch_command: null,
  },
];

const row = (r: Partial<ModRow> & Pick<ModRow, "id" | "name">): ModRow => ({
  folder: `${r.name} [gb-${r.gb_mod_id ?? 0}]`,
  enabled: true,
  disk: "enabled",
  variant: null,
  gb_mod_id: 700_000 + r.id,
  gb_file_id: 1_400_000 + r.id,
  group: null,
  version: "1.0",
  size_bytes: 24_000_000,
  installed_at: NOW - r.id * DAY,
  root_category: "Character Skins",
  root_category_id: 30305,
  category: "Ellen",
  category_id: 43004,
  sensitive: false,
  preview: art("#5b8def"),
  ...r,
});

/** Three versions of one sword pack: one group, the second version on. */
const swords: ModRow[] = ["Blue Blade", "Red Blade", "Gold Blade"].map((variant, i) =>
  row({
    id: 10 + i,
    name: "Sample Sword Pack",
    variant,
    enabled: i === 1,
    disk: i === 1 ? "enabled" : "disabled",
    folder: `${i === 1 ? "" : "DISABLED "}Sample Sword Pack [gb-553203] (${variant})`,
    gb_mod_id: 553203,
    gb_file_id: 1314110,
    group: "553203:1314110",
    category: "Anby",
    category_id: 30336,
    size_bytes: 12_000_000 + i * 1_000_000,
    preview: art("#e0a040"),
  }),
);

export const libraries: Record<string, LibraryView> = {
  zzz: {
    game: "zzz",
    problem: null,
    managed: [
      row({ id: 1, name: "Casual Outfit", gb_mod_id: 688491, preview: art("#43cbbe") }),
      row({ id: 2, name: "Swimsuit Recolor", enabled: false, disk: "disabled", sensitive: true, category: "Nicole" }),
      ...swords,
      row({ id: 3, name: "Old Install", root_category: null, root_category_id: null, category: null, preview: null }),
    ],
    unmanaged: [
      { name: "Hand Folder", path: "Hand Folder", enabled: true, is_dir: true },
      { name: "Loose Tweak", path: "DISABLED Loose Tweak", enabled: false, is_dir: true },
    ],
  },
  wuwa: {
    game: "wuwa",
    problem: null,
    managed: [
      row({ id: 20, name: "Coat Retexture", root_category: "Skins", category: "Rover", preview: art("#9b7bff") }),
    ],
    unmanaged: [],
  },
};

export const preferences: Preferences = {
  theme: "dark",
  show_sensitive: false,
  blur_sensitive: true,
  onboarded: true,
};

export const settings: Settings = {
  database: "C:\\Users\\player\\AppData\\Roaming\\GMM\\gmm.db",
  xxmi_root: null,
  confirmed_xxmi_exe: null,
  manual_folders: [{ game: "wuwa", folder: "D:\\Games\\Wuthering Waves" }],
  http_cache_entries: 120,
  http_cache_bytes: 2_400_000,
  download_cache_entries: 6,
  download_cache_bytes: 180_000_000,
};

const size = (bytes: number, files = 10) => ({ bytes, files, truncated: false });
const usage = (id: number, folder: string, bytes: number, enabled = true) => ({
  mod_ref: { type: "managed" as const, id, folder },
  enabled,
  size: size(bytes),
});

export const storage: StorageView = {
  games: [
    {
      game: "zzz",
      problem: null,
      storage: {
        game: "zzz",
        managed: [
          usage(1, "Casual Outfit [gb-688491]", 131_000_000),
          usage(2, "DISABLED Swimsuit Recolor", 40_000_000, false),
          usage(10, "DISABLED Sample Sword Pack (Blue Blade)", 12_000_000, false),
          usage(11, "Sample Sword Pack (Red Blade)", 13_000_000),
        ],
        unmanaged: [{ mod_ref: { type: "unmanaged", path: "Hand Folder" }, enabled: true, size: size(90_000_000) }],
        managed_total: size(196_000_000, 40),
        unmanaged_total: size(90_000_000, 12),
        other: size(2_000_000, 3),
        free_bytes: 400_000_000_000,
      },
    },
    { game: "wuwa", problem: "XXMI's WWMI folder is missing.", storage: null },
  ],
  cache: {
    downloads: size(180_000_000, 6),
    extract: size(0, 0),
    images: size(52_000_000, 300),
    http_entries: 120,
    http_bytes: 2_400_000,
    free_bytes: 400_000_000_000,
  },
};

const file = (f: Partial<ModFile> & Pick<ModFile, "id" | "name">): ModFile => ({
  size: 6_300_000,
  date_added: NOW - 40 * DAY,
  download_count: 1200,
  download_url: `https://gamebanana.com/dl/${f.id}`,
  md5: null,
  version: "1.0",
  description: "The full pack.",
  analysis_result: "ok",
  av_result: "clean",
  archived: false,
  ...f,
});

export function summary(id: number, name: string, extra: Partial<ModSummary> = {}): ModSummary {
  return {
    id,
    game_id: 19567,
    name,
    submitter: { id: 1000 + id, name: `author${id}`, avatar_url: null, profile_url: null },
    root_category: { id: 30305, name: "Character Skins" },
    category: { id: 30336, name: "Anby" },
    version: "1.0",
    date_added: NOW - 30 * DAY,
    date_modified: NOW - 3 * DAY,
    date_updated: null,
    images: [image("#5b8def"), image("#e0a040"), image("#43cbbe")],
    stats: { likes: 320, views: 41_000, posts: 12, downloads: null },
    featured: false,
    obsolete: false,
    visibility: "show",
    has_content_ratings: false,
    profile_url: `https://gamebanana.com/mods/${id}`,
    ...extra,
  };
}

export const catalogPage: CatalogPage = {
  page: {
    items: [
      summary(553203, "Sample Sword Pack"),
      summary(700101, "Street Jacket", { category: { id: 43004, name: "Ellen" } }),
      summary(700102, "Night Dress", { visibility: "warn" }),
      summary(700103, "Recolored Boots", { stats: { likes: 12, views: 900, posts: 1, downloads: null } }),
      summary(700104, "A Very Long Mod Name That Keeps Going To Check How Cards Wrap Their Titles"),
      summary(700105, "Summer Hat"),
    ],
    total: 6,
    page: 1,
    per_page: 30,
    is_last: true,
  },
  hidden: 1,
};

export const featured: FeaturedMod[] = [
  { summary: summary(700201, "Trending Outfit"), period: "week" },
  { summary: summary(700202, "Popular Weapon"), period: null },
];

export const categories: Category[] = [
  { id: 30336, name: "Anby", item_count: 130, subcategory_count: 0, icon_url: null, obsolete: false },
  { id: 43004, name: "Ellen", item_count: 210, subcategory_count: 0, icon_url: null, obsolete: false },
];

export const modProfile: ModProfile = {
  summary: summary(553203, "Sample Sword Pack", { version: "1.2", date_updated: NOW - 3 * DAY }),
  description_html:
    '<p>Eight swords for <b>Anby</b>. See <a href="#gmm-link:0123456789abcdef0123456789abcdef">the guide</a>.</p><ul><li>Blue</li><li>Red</li></ul>',
  license_html: "<p>Credit me if you share edits.</p>",
  license: { yes: ["Use in videos"], ask: ["Edit and share"], no: ["Sell"] },
  credits: [{ name: "Key Authors", authors: [{ name: "author553203", role: "Model edits", member_id: 1 }] }],
  files: {
    files: [
      file({ id: 1314110, name: "sample_swords_v12.zip", version: "1.2" }),
      file({ id: 1314111, name: "sample_swords_lite.7z", size: 2_100_000, description: null, version: null }),
    ],
    archived_files: [file({ id: 1200000, name: "sample_swords_v1.zip", archived: true, date_added: NOW - 200 * DAY })],
  },
  updates_count: 1,
  is_trashed: false,
  is_withheld: false,
};

export const modUpdates: Page<ModUpdate> = {
  items: [
    {
      id: 1,
      title: "Version 1.2",
      version: "1.2",
      date_added: NOW - 3 * DAY,
      changelog: [{ text: "Fixed the gold blade glow", category: "Bugfix" }],
      text_html: "<p>Smaller files.</p>",
      file_ids: [1314110],
    },
  ],
  total: 1,
  page: 1,
  per_page: 10,
  is_last: true,
};

export const updates: ModUpdateInfo[] = [
  {
    game: "zzz",
    gb_mod_id: 688491,
    name: "Casual Outfit",
    installed_ids: [1],
    installed_file_id: 1_400_001,
    installed_file_date: NOW - 60 * DAY,
    newest: file({ id: 1_400_900, name: "casual_outfit_v2.zip", version: "2.0", date_added: NOW - DAY }),
  },
];

export const jobs: Job[] = [
  {
    id: 1,
    action: "install",
    game: "zzz",
    mod_id: 700101,
    mod_name: "Street Jacket",
    author: "author700101",
    file_name: "street_jacket.zip",
    file_size: 48_000_000,
    image: null,
    versions: 1,
    state: { type: "downloading", downloaded: 20_000_000, total: 48_000_000 },
    queued_at: NOW - 60,
    finished_at: null,
  },
  {
    id: 2,
    action: "install",
    game: "wuwa",
    mod_id: 700300,
    mod_name: "Broken Upload",
    author: null,
    file_name: "broken.rar",
    file_size: 1_000,
    image: null,
    versions: 1,
    state: { type: "failed", message: "the archive is damaged" },
    queued_at: NOW - 600,
    finished_at: NOW - 590,
  },
];

export const profiles: ProfileInfo[] = [{ id: 1, game: "zzz", name: "Beach day", mods: 3, updated_at: NOW - DAY }];

export const keybindings: KeyBindings = {
  bindings: [{ ini: "mod.ini", name: "KeySwap", keys: ["no_modifiers VK_LEFT"], back: [], kind: "cycle" }],
  notes: [],
};

export const installPlan: InstallPlan = {
  game: "zzz",
  mod_id: 553203,
  mod_name: "Sample Sword Pack",
  file: modProfile.files.files[0],
  other_files: [modProfile.files.files[1]],
  archive_bytes: 6_300_000,
  from_cache: true,
  kind: "3dmigoto",
  versions: [
    {
      name: "Blue Blade",
      folder: "Blue Blade",
      files: 10,
      bytes: 12_000_000,
      installed: 10,
      blocked: [],
      warnings: [],
    },
    { name: "Red Blade", folder: "Red Blade", files: 10, bytes: 13_000_000, installed: 11, blocked: [], warnings: [] },
    {
      name: "Gold Blade",
      folder: "Gold Blade",
      files: 10,
      bytes: 14_000_000,
      installed: 12,
      blocked: [],
      warnings: [],
    },
    {
      name: "Void Blade",
      folder: "Void Blade",
      files: 11,
      bytes: 11_000_000,
      installed: null,
      blocked: [],
      warnings: [],
    },
    {
      name: "Spirit Blade",
      folder: "Spirit Blade",
      files: 12,
      bytes: 17_000_000,
      installed: null,
      blocked: ["tools/setup.exe"],
      warnings: [],
    },
  ],
  version_conflicts: [{ a: 3, b: 4, hashes: 6 }],
  install_to: "ZZMI\\Mods\\GMM",
};

export function uninstallPreview(keys: { type: string; id?: number; path?: string }[]): UninstallItem[] {
  return keys.map((k) => ({
    key: k.type === "managed" ? { type: "managed", id: k.id ?? 0 } : { type: "unmanaged", path: k.path ?? "" },
    name: k.type === "managed" ? `Mod #${k.id}` : (k.path ?? ""),
    folder: `C:\\XXMI\\ZZMI\\Mods\\${k.type === "managed" ? `GMM\\mod-${k.id}` : k.path}`,
    files: k.type === "managed" ? null : 4,
    bytes: 12_000_000,
    truncated: false,
    hand_installed: k.type !== "managed",
  }));
}
