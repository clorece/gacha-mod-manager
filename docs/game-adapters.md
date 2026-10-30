# Game adapters, detection & XXMI setup states

Every supported game is loaded by an **XXMI importer** (ZZMI for Zenless Zone Zero, WWMI for
Wuthering Waves). A game's adapter says where it is on GameBanana, how to find it on disk,
which importer folder its mods go in, and how to recognise a mod in an archive.

> Facts marked "XXMI source" were learned by reading XXMI Launcher (GPL-3.0); "observed" means
> seen on a real Windows 11 install. They're written here in our own words. Implement from this
> page, not from XXMI's code (see [licensing.md](licensing.md)).

## Adapter contract

Each game is a TOML manifest in [`crates/gmm-adapters/games/`](../crates/gmm-adapters/games/),
compiled into GMM and validated at startup. Manifests are data only:

- `[game]` id and names, `[gamebanana]` game id and root categories, `[xxmi]` importer and
  expected `d3dx.ini` target, `[detect]` exe names, required folders, Steam appid and Epic
  names, `[[mod_kind]]` how to spot a mod, `[conflicts]` libraries to ignore.
- A folder in an archive is a mod when it directly holds an `.ini` containing
  `[TextureOverride` or `[ShaderOverride` and no folder above it does. Several such folders are
  variants.
- Unknown fields are rejected, and every path is validated.
- A manifest can't add network hosts, filesystem roots or launch arguments.

## Game detection

Every candidate folder must have the game's marker files (an exe and required folders).

| Source | How | Status |
|---|---|---|
| **Steam** | `SteamPath` (registry; `~/.steam/steam` on Linux) → `libraryfolders.vdf` → `appmanifest_<appid>.acf` → the game folder | ✅ observed (WuWa) |
| **HoYoPlay** | `%APPDATA%\Cognosphere\HYP\<version>\data\gamedata.dat`: only `"installPath"` values are read | ✅ observed (ZZZ), Windows only |
| **Epic** | `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests\*.item` (name and install location only) | ✅ fixture-tested only |
| **Manual** | Settings → Games, or `gmm config add-folder game=PATH` | ✅ |
| Kuro launcher | `kr_starter_game.json` (XXMI source) | ⏳ not built |

GMM never reads game logs, even though they would reveal install paths, because they can hold
account details.

## XXMI detection & setup states

### Finding XXMI

Tried in order; the first valid folder wins:

1. A folder the user picked (Settings → XXMI folder, or `gmm config set-xxmi PATH`).
2. `%APPDATA%\XXMI Launcher` (the default).
3. XXMI's MSI uninstall entry in the registry (`InstallLocation` only).

A valid XXMI folder has `XXMI Launcher Config.json` and `Resources/Bin/XXMI Launcher.exe`.
XXMI's exe isn't code-signed, so GMM shows its path and asks the user to confirm it before the
first launch, and again if it changes.

### Per-importer state

Checked in this order; the first failure decides, so the user sees the one next step that
matters.

| State | Meaning | Fix GMM shows |
|---|---|---|
| `XxmiNotFound` | No XXMI folder found | Install XXMI from its releases page |
| `ImporterMissing` | The game's importer isn't installed in XXMI | In XXMI, tick the game under **Select Games To Mod:** and press **Install** |
| `ImporterBroken` | `d3dx.ini` targets the wrong exe or doesn't load `Mods` | Reinstall or update the importer in XXMI |
| `GamePathNotSet` | XXMI has no game folder | XXMI settings → **General** → **Game Folder** |
| `PathMismatch` | XXMI's game folder isn't the game | Same, naming the wrong folder |
| `LaunchModeMismatch` | XXMI's **Game Launch** setting won't work for this copy | XXMI settings → **General** → **Game Launch** |
| `Ready` | All good | — |

**Game Launch rules:** a Steam copy needs **Steam** (WuWa's Steam copy also works with
**Direct**), an Epic copy needs **Epic Games**, and HoYoPlay or manual installs need
**Direct**. **Manual** always passes. Why: with Direct, XXMI starts the exe itself, and ZZZ's
Steam copy refuses to run that way (observed).

### Reading the XXMI config safely

`XXMI Launcher Config.json` holds secrets (`github_token`, `proxy`, signatures). GMM parses it
into a struct that only declares `config_version` and, per importer, `game_folder`,
`importer_folder`, `game_launch` and `launch_count`. Everything else is skipped without being
stored. The file is never logged, copied or written to, and tests check that planted canary
secrets never show up.

## The games

| | Zenless Zone Zero | Wuthering Waves |
|---|---|---|
| GameBanana id | 19567 | 20357 |
| Importer | ZZMI, `d3dx.ini target = ZenlessZoneZero.exe` | WWMI, `target = Client-Win64-Shipping.exe` |
| Folder markers | `ZenlessZoneZero.exe` + `ZenlessZoneZero_Data/` | `Wuthering Waves.exe` + `Client/` + `Engine/` |
| Steam | appid 4162040, game in `games/ZenlessZoneZero Game`; Game Launch = Steam | appid 3513350; Game Launch = Steam or Direct |
| Other sources | HoYoPlay; Game Launch = Direct | — |

Game settings XXMI changes (ZZZ's animation and image quality, WuWa's ray tracing and LOD) stay
with XXMI; GMM never touches them. WuWa's `~mods` pak mods are out of scope (XXMI doesn't load
them).

## Adding a game

XXMI also supports Endfield (EFMI), Star Rail (SRMI), Genshin (GIMI) and Honkai Impact 3rd
(HIMI). For each:

1. Find the GameBanana game id and root categories.
2. Record the folder markers, Steam appid and other launcher sources from a real install.
3. Copy [`crates/gmm-adapters/template/game.toml`](../crates/gmm-adapters/template/game.toml)
   to `games/<id>.toml` and fill in its TODOs. Its header lists every other file a new game
   touches (`BUILTIN` in `registry.rs`, tests, banner).
4. The conformance suite (`crates/gmm-core/tests/conformance.rs`) runs against every manifest
   automatically: detect, setup state, install, switch, conflicts, storage, uninstall.
5. Review the new paths; they become filesystem roots.
