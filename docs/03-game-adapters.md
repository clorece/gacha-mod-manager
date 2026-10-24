# 03 — Game adapters, detection & XXMI setup states

Every supported game is loaded by an **XXMI importer**. An adapter ties together: the GameBanana
catalogue, how to find the game on disk, which importer folder its mods go in, and how to read a
mod archive.

> **Provenance:** facts marked "XXMI source" were learned by reading XXMI Launcher (GPL-3.0).
> Facts marked "observed" come from the dev machine (`research/phase0-observations.md`). These
> tables restate interoperability facts in our own words. Implement from them, not from XXMI's
> code (see [06](06-licensing-and-provenance.md)).

## Adapter contract

Implemented in `crates/gmm-adapters`. Each game is a TOML manifest in
[`crates/gmm-adapters/games/`](../crates/gmm-adapters/games/), compiled into the binary and
validated when the registry is built (`Registry::builtin()`). Manifests are **data only**:

- Sections: `[game]` (id, names, engine, risk notice), `[gamebanana]` (game id, root
  categories with a tag and whether subcategories are characters), `[xxmi]` (importer +
  expected `d3dx.ini` target), `[detect]` (exe names, required children, folder names,
  `[detect.steam]` appid + sub-path, `[detect.epic]` display names), `[[mod_kind]]`,
  `[conflicts]`.
- Unknown fields are rejected, so a typo can't silently switch a rule off. Every path is
  validated with fsguard's `RelPath` rules. Ids, GameBanana ids, importers and Steam appids
  must be unique across games.
- **What a manifest can't do:** add network hosts, filesystem roots, or launch arguments. The
  XXMI command is always `--nogui --xxmi <importer>`, built in code (rule R7).

Code hooks, for formats a manifest can't express, get added per game when one is needed. None
is needed for ZZZ or WuWa yet.

## Game detection (Windows)

Implemented in `crates/gmm-detect` (`gmm detect`). Every candidate must pass the adapter's
folder markers (one exe name plus every required child); if a folder fails, the manifest's
typical sub-folder names are tried (launchers often install into `<chosen>/<Game> Game`).
Results are canonicalized and deduped.

| Source | How | Status |
|---|---|---|
| **Steam** | `HKCU\Software\Valve\Steam\SteamPath` → `steamapps/libraryfolders.vdf` → each library's `appmanifest_<appid>.acf` (`installdir`, must be one plain folder name) → `common/<installdir>/<steam_subpath>` | ✅ verified on the dev machine (WuWa 3513350). ZZZ 4162040's Steam copy nests the game in `games/ZenlessZoneZero Game` |
| **HoYoPlay** (native ZZZ, Genshin, Star Rail, HI3) | `%APPDATA%\Cognosphere\HYP\<version>\data\gamedata.dat`: HoYoPlay's own binary format with JSON records embedded. Only `"installPath"` values are extracted; each counts only if it passes a game's markers. Most records hold default paths for uninstalled games, which fail the check on their own | ✅ verified on the dev machine (ZZZ at `D:\HoYoverse\ZenlessZoneZero Game`). The file stays readable while HoYoPlay runs (opened in shared mode) |
| **Epic** | `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON: only `DisplayName`, `InstallLocation` read) | ✅ implemented, fixture-tested only (no machine with Epic yet) |
| **Manual** | `gmm detect/status --folder game=PATH`; later a folder picker. Validated against markers; a failing folder produces a note | ✅ implemented (not yet remembered between runs: needs the database, Phase 2) |
| **XXMI config** | `Importers.<IMP>.Importer.game_folder` | Used by the setup check, not as a detection source |
| **Kuro launcher** (native WuWa) | `%APPDATA%\KRLauncher\**\kr_starter_game.json` → `path` (XXMI source) | ⏳ not implemented; needs a machine that has it |
| **Drive scan** | User-initiated only, depth-limited, folder-name match | ⏳ not implemented |

**Not used, on purpose:** game log files (e.g. ZZZ's `Player.log`). XXMI reads them to find the
install, but logs can contain account details. GMM doesn't need them.

## XXMI detection & setup states

### Finding XXMI
1. Default root `%APPDATA%\XXMI Launcher` (observed).
2. MSI install record (observed): `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\*`
   entries with `DisplayName = "XXMI Launcher"` carry `InstallLocation` (the root) and
   `DisplayVersion`. Read-only, and only those two values.
3. Root recorded in GMM settings (user-picked for portable installs).

A folder counts as an XXMI root when it has `XXMI Launcher Config.json` **and**
`Resources/Bin/XXMI Launcher.exe` (observed; its version resource says ProductName
`XXMI Launcher`, CompanyName `SpectrumQT`, FileVersion `2.4.9.0`).

**The exe is not code-signed** (observed), so GMM can't verify it cryptographically. Before
the first launch GMM shows the exe's path and version and asks the user to confirm. It
re-asks whenever the path changes, and launches only that exe with fixed arguments (security
rule R7, threat T5).

**Launch command (observed):** XXMI's own "Add Desktop Shortcut" creates
`<IMP> Quick Start.lnk` → `Resources\Bin\XXMI Launcher.exe --nogui --xxmi <IMP>`
(e.g. `--xxmi ZZMI`). GMM's **Launch with XXMI** runs the same command.

### Per-importer state (computed per game)

Implemented in `crates/gmm-xxmi` (`gmm status`). Checks run in this order; the first failing
one decides the state, so the user gets the one next step that matters.

| State | How it's detected | What GMM shows |
|---|---|---|
| `XxmiNotFound` | No valid root found | Link to XXMI's releases, install steps |
| `ImporterMissing` | `<root>/<IMP>/` missing, or missing `d3d11.dll`, `d3dx.ini`, or the core ini that `d3dx.ini` includes | "Under **Select Games To Mod:**, tick <game> and press **Install**" |
| `ImporterBroken` | `d3dx.ini` `target` ≠ the manifest's `d3dx_target`, or no `include_recursive = Mods` | "Reinstall or update <IMP> in XXMI" |
| `GamePathNotSet` | `game_folder` in XXMI's config is empty | Settings → **General** → **Game Folder**, with the folder GMM detected (or **Detect Installations**) |
| `PathMismatch` | XXMI's `game_folder` fails the game's folder markers | Same steps, naming the wrong folder |
| `LaunchModeMismatch` | XXMI's **Game Launch** can't work for how this copy is started (rules below) | Settings → **General** → **Game Launch** → the right option |
| `Ready` | All checks pass | The exact launch command; notes (e.g. "XXMI hasn't started this game yet") |

**Launch mode rules.** XXMI's per-game **Game Launch** setting
(`Importers.<IMP>.Importer.game_launch`) offers Direct, Steam, Epic Games, Custom and Manual
(config values `DIRECT`, `STEAM`, `EPIC_GAMES`, `CUSTOM`, `MANUAL`). The source of XXMI's
game folder is found by matching it against detected installs:

| Install source | Works | Fix if not |
|---|---|---|
| Steam | `STEAM`, `MANUAL` | set to **Steam** |
| Epic | `EPIC_GAMES`, `MANUAL` | set to **Epic Games** |
| HoYoPlay, manual | `DIRECT`, `CUSTOM`, `MANUAL` | set to **Direct** |
| not a detected install | not checked (note shown) | — |

Why (observed 2026-10-23): with **Direct**, XXMI starts the game exe itself. Injection succeeds,
but the **Steam** copies of ZZZ and WuWa then refuse to run ("please launch from Steam") and
exit. The HoYoPlay copy of ZZZ works with Direct, and mods show in game.

Mods can still be **downloaded** in any state where the importer exists. They are just inactive
until XXMI runs. In `XxmiNotFound` / `ImporterMissing`, installs are queued and placed once the
`Mods/` folder appears.

Extra info shown per importer (read-only):
- XXMI DLL version from `d3d11.dll`'s version resource (observed `1.2.2.0`).
- Importer version from the importer's **core ini**, found through `d3dx.ini`'s own
  `include = Core\<IMP>\…` line rather than a guessed name (observed: ZZMI
  `Core\ZZMI\main.ini` with `global $version = 1.50`; WWMI `Core\WWMI\WuWa-Model-Importer.ini`
  with `global $wwmi_version = 1.10`; `$required_*` keys ignored).
- Enabled and disabled mod counts (top-level folders in `Mods/`, `DISABLED*` = disabled), and the
  total size of `Mods/`.

**States seen on the dev machine:** ZZZ went `GamePathNotSet` → `LaunchModeMismatch` (Steam
copy) → `Ready` (after moving to a HoYoPlay install on D:). WuWa went `ImporterMissing` →
`LaunchModeMismatch` (Steam, Game Launch still Direct). Every state has a fixture test.

### Reading the XXMI config safely

`XXMI Launcher Config.json` contains secrets (`Launcher.github_token`, `Launcher.proxy`,
`Security.user_signature`, `*_signature`). GMM (`gmm_xxmi::XxmiConfig`) deserializes it into a
struct that **declares only** `Launcher.config_version`, and per importer
`Importer.game_folder`, `Importer.importer_folder`, `Importer.game_launch` and
`Importer.launch_count`. serde skips all other fields without storing them. Parse errors never
echo file content. The file is never logged, copied, cached or sent anywhere, and GMM never
writes to it. Tests plant canary secrets in a fixture config and check they never appear in
output, JSON or `Debug`.

### In-app setup guide (our own wording)

Bold names are XXMI's own English labels, taken from its locale files (`Locale/Strings/EN_*.toml`,
XXMI 2.4.9). `gmm status` already prints these steps per state. Our own screenshots go next to
each step before the setup guide ships in the app (see "Before Phase 2" in the roadmap).

**Install XXMI**
1. Open XXMI's releases page (link: `https://github.com/SpectrumQT/XXMI-Launcher/releases`).
2. Download the installer (`.msi`) or the portable `.zip`.
3. Install it, or extract it somewhere outside *Program Files*, then start XXMI once.
4. Back in GMM, press **Re-check** (or pick the folder if you used the portable version).

**Set up a game in XXMI**
1. Open XXMI Launcher.
2. Under **Select Games To Mod:**, tick your game (ZZMI for ZZZ, WWMI for WuWa) and press
   **Install**.
3. Open the settings, **General** tab. Check **Game Folder** (or press **Detect
   Installations**). GMM shows the folder it found, with a copy button.
4. Same tab: set **Game Launch** to match your copy: **Steam** for Steam, **Epic Games** for
   Epic, **Direct** for HoYoPlay and other launchers.
5. Press **Start** once, so XXMI finishes setup and applies the game settings mods need.
6. Optional: **Add Desktop Shortcut** creates an "<importer> Quick Start" shortcut that skips
   XXMI's window.
7. Back in GMM, press **Re-check**.

## ZZZ

| Item | Value | Source |
|---|---|---|
| GameBanana | id 19567. Roots: Character Skins (30305), Bangboo Skins (30702), UI (30395), Other/Misc (29874). Subcategory = character | API, verified |
| XXMI importer | ZZMI (`<XXMI>/ZZMI/`), `d3dx.ini target = ZenlessZoneZero.exe` | observed |
| Game folder markers | `ZenlessZoneZero.exe` (or `ZenlessZoneZeroBeta.exe`) + `ZenlessZoneZero_Data/` | observed + XXMI source |
| Steam | appid 4162040, sub-path `games/ZenlessZoneZero Game`. Needs Game Launch = Steam | observed |
| HoYoPlay | `gamedata.dat` record with `gameBiz = nap_global`; works with Game Launch = Direct; mods confirmed in game | observed |
| Library mods (ignore in conflicts) | ZZMI's own `Core/ZZMI/Libraries` (SlotFix, TTLib, HP bar) are outside `Mods/`. Common Mods-folder libraries **[collect list]** | observed |
| Settings XXMI applies | High-precision character animation off, custom image quality (in `GENERAL_DATA.bin`) | XXMI source. GMM doesn't touch them |

## WuWa

| Item | Value | Source |
|---|---|---|
| GameBanana | id 20357. Roots: Skins (29524), UI (29496), Other/Misc (29493) | API, verified |
| XXMI importer | WWMI (`<XXMI>/WWMI/`), `d3dx.ini target = Client-Win64-Shipping.exe`, core ini `Core\WWMI\WuWa-Model-Importer.ini` (`$wwmi_version`) | observed |
| Game folder markers | `Wuthering Waves.exe` + `Client/` + `Engine/`; process `Client/Binaries/Win64/Client-Win64-Shipping.exe` | observed |
| Steam | appid 3513350, sub-path empty (the Steam folder is the game folder). Needs Game Launch = Steam | observed |
| Library mods | **[collect list]** | |
| Settings XXMI applies | Ray tracing off, mesh LOD override in `Client/Config/UserEngine.ini`, DX11 | XXMI source. GMM doesn't touch them |
| Pak mods (`~mods`) | Out of scope. They aren't loaded by XXMI | decision |

## Classifier rules (shared)

- `migoto`: one or more `.ini` files containing `[TextureOverride` or `[ShaderOverride`. The mod
  root is the shallowest directory containing all of them, which strips wrapper folders.
- **Variants:** if two or more sibling directories each qualify as a full mod root, the archive
  is multi-variant. The user picks one or more. Preview images inside each variant
  (`preview.*`, `*.png/jpg` at the variant root) are shown in the picker.
- Anything else → `Unknown` → manual review. Never guessed.

## Adding a game

XXMI already supports Endfield (EFMI), Star Rail (SRMI), Genshin (GIMI) and HI3 (HIMI). For each:

1. Find the GameBanana game id and root categories.
2. Record the folder markers, Steam appid/sub-path and launcher sources (observe a real install).
3. Write `crates/gmm-adapters/games/<id>.toml` and add it to `BUILTIN` in `registry.rs`. Hook
   code is rarely needed.
4. Add fixtures: a fake install tree, a fake XXMI importer folder in each setup state, and sample
   mods. The shared conformance suite (detect, states, classify, install, toggle, uninstall)
   runs against every adapter automatically.
5. Review the new paths (they become filesystem roots).

Observed: Endfield is in the dev machine's Steam library (appid 4732690, not installed yet), so
it's the natural third game.
