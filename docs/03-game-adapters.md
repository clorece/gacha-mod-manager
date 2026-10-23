# 03 — Game adapters, detection & XXMI setup states

Every supported game is loaded by an **XXMI importer**. An adapter ties together: the GameBanana
catalogue, how to find the game on disk, which importer folder its mods go in, and how to read a
mod archive.

> **Provenance:** facts marked "XXMI source" were learned by reading XXMI Launcher (GPL-3.0).
> Facts marked "observed" come from the dev machine (`research/phase0-observations.md`). These
> tables restate interoperability facts in our own words. Implement from them, not from XXMI's
> code (see [06](06-licensing-and-provenance.md)).

## Adapter contract

A TOML manifest (data), plus an optional Rust hook module, compiled into the binary:

```rust
pub trait GameAdapter: Send + Sync {
    fn manifest(&self) -> &GameManifest;
    fn detect(&self, ctx: &DetectCtx) -> Vec<GameInstallCandidate>; // default: manifest-driven
    fn validate_game_folder(&self, dir: &ScopedPath) -> Result<()>;  // default: markers
    fn classify(&self, tree: &ExtractedTree) -> Classification;      // migoto / unknown, variants
    fn library_mods(&self) -> &[LibraryModRule];                     // conflict-ignore list
}
```

Manifest sections: `[game]`, `[gamebanana]`, `[xxmi]` (importer id + expected `d3dx.ini`
target), `[detect]` (Steam appid + sub-path, launcher config files, Epic names, folder markers),
`[[mod_kind]]`, `[conflicts]` (ignore rules). See [examples/](examples/).

## Game detection (Windows)

Sources are tried in order. All candidates are canonicalized and deduped, and each one is
validated against the adapter's folder markers.

| Source | How | Notes |
|---|---|---|
| **Steam** | `HKCU\Software\Valve\Steam\SteamPath` → `steamapps/libraryfolders.vdf` → `appmanifest_<appid>.acf` (`installdir`) → `common/<installdir>/<steam_subpath>` | Observed: ZZZ 4162040, WuWa 3513350. ZZZ's game sits at `games/ZenlessZoneZero Game` inside the Steam folder (HoYoPlay wrapper) |
| **HoYoPlay** (native ZZZ) | HoYoPlay's data under `%APPDATA%\Cognosphere\HYP\` | Which file holds the install path is **[verify]** |
| **Kuro launcher** (native WuWa) | `%APPDATA%\KRLauncher\**\kr_starter_game.json` → `path` | XXMI source; **[verify]** on a machine that has it |
| **Epic** | `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON: `DisplayName`, `InstallLocation`) | Both games are on Epic |
| **XXMI config** | `Importers.<IMP>.Importer.game_folder` (only this key) | Empty until the game has been launched through XXMI |
| **Manual** | Folder picker; validated against markers | Always available |
| **Drive scan** | User-initiated only, depth-limited, folder-name match | Never automatic |

**Not used, on purpose:** game log files (e.g. ZZZ's `Player.log`). XXMI reads them to find the
install, but logs can contain account details. GMM doesn't need them.

## XXMI detection & setup states

### Finding XXMI
1. Default root `%APPDATA%\XXMI Launcher` (observed).
2. Root recorded in GMM settings (user-picked for portable installs).
3. MSI uninstall registry entry **[verify]**.

A folder counts as an XXMI root when it has `XXMI Launcher Config.json` **and**
`Resources/Bin/XXMI Launcher.exe` **[verify exe path]**.

### Per-importer state (computed per game)

| State | How it's detected | What GMM shows |
|---|---|---|
| `XxmiNotFound` | No root found | "Install XXMI" card |
| `ImporterMissing` | `<root>/<IMP>/` missing, or no `d3d11.dll` / `d3dx.ini` / `Core/<IMP>/main.ini` | "Install the <IMP> importer in XXMI" steps |
| `ImporterBroken` | Files present but `d3dx.ini` has no `include_recursive = Mods`, or `target` ≠ game exe | "Reinstall <IMP> in XXMI" steps |
| `GamePathNotSet` | Importer OK, but `game_folder` in XXMI config is empty | "Open XXMI, select <game>, confirm the game folder, launch once" + the folder GMM detected, with a copy button |
| `PathMismatch` | XXMI's `game_folder` isn't any install GMM found, or fails validation | Warning, both paths shown, fix steps |
| `Ready` | All checks pass | Mods page enabled, **Launch with XXMI** enabled |

Mods can still be **downloaded** in any state where the importer exists. They are just inactive
until XXMI runs. In `XxmiNotFound` / `ImporterMissing`, installs are queued and placed once the
`Mods/` folder appears.

Extra info shown per importer (read-only): XXMI DLL version (`d3d11.dll` FileVersion, observed
`1.2.2.0`), importer version (`global $version` in `Core/<IMP>/main.ini`, observed ZZMI `1.50`),
mod count, and `Mods/` size.

**The dev machine shows two of these states:** ZZZ is in `GamePathNotSet` (ZZMI installed, never
launched through XXMI), and WuWa is in `ImporterMissing` (no `WWMI/` folder). These become the
first test fixtures.

### Reading the XXMI config safely

`XXMI Launcher Config.json` contains secrets (`Launcher.github_token`, `Launcher.proxy`,
`Security.user_signature`, `*_signature`). GMM deserializes it into a struct that **declares only**:
`Launcher.config_version`, and per importer `Importer.game_folder`, `Importer.importer_folder`,
`Importer.game`. serde drops all other fields. The file is never logged, copied, cached or sent
anywhere, and GMM never writes to it.

### In-app setup guide (our own wording; exact XXMI button labels **[verify]**)

**Install XXMI**
1. Open XXMI's releases page (link: `https://github.com/SpectrumQT/XXMI-Launcher/releases`).
2. Download the installer (`.msi`) or the portable `.zip`.
3. Install it, or extract it somewhere outside *Program Files*, then start XXMI once.
4. Back in GMM, press **Re-check** (or pick the folder if you used the portable version).

**Set up a game in XXMI**
1. Open XXMI Launcher.
2. Open the importer manager and enable the importer for your game (ZZMI for ZZZ, WWMI for WuWa).
3. Select the game and press **Install**.
4. Open the game's settings in XXMI and check that the game folder is right. GMM shows the folder
   it found, with a copy button.
5. Launch the game once from XXMI, so it finishes setup and applies the game settings mods need.
6. Back in GMM, press **Re-check**.

## ZZZ

| Item | Value | Source |
|---|---|---|
| GameBanana | id 19567. Roots: Character Skins (30305), Bangboo Skins (30702), UI (30395), Other/Misc (29874). Subcategory = character | API, verified |
| XXMI importer | ZZMI (`<XXMI>/ZZMI/`), `d3dx.ini target = ZenlessZoneZero.exe` | observed |
| Game folder markers | `ZenlessZoneZero.exe` (or `ZenlessZoneZeroBeta.exe`) + `ZenlessZoneZero_Data/` | observed + XXMI source |
| Steam | appid 4162040, sub-path `games/ZenlessZoneZero Game` | observed |
| Library mods (ignore in conflicts) | ZZMI's own `Core/ZZMI/Libraries` (SlotFix, TTLib, HP bar) are outside `Mods/`. Common Mods-folder libraries **[collect list]** | observed |
| Settings XXMI applies | High-precision character animation off, custom image quality (in `GENERAL_DATA.bin`) | XXMI source. GMM doesn't touch them |

## WuWa

| Item | Value | Source |
|---|---|---|
| GameBanana | id 20357. Roots: Skins (29524), UI (29496), Other/Misc (29493) | API, verified |
| XXMI importer | WWMI (`<XXMI>/WWMI/`), expected `d3dx.ini target = Client-Win64-Shipping.exe` **[verify]** | XXMI source |
| Game folder markers | `Wuthering Waves.exe` + `Client/` + `Engine/`; process `Client/Binaries/Win64/Client-Win64-Shipping.exe` | observed |
| Steam | appid 3513350, sub-path empty (the Steam folder is the game folder) | observed |
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
3. Write `games/<id>.toml` mapping to the importer. Hook code is rarely needed.
4. Add fixtures: a fake install tree, a fake XXMI importer folder in each setup state, and sample
   mods. The shared conformance suite (detect, states, classify, install, toggle, uninstall)
   runs against every adapter automatically.
5. Review the new paths (they become filesystem roots).

Observed: Endfield is in the dev machine's Steam library (appid 4732690, not installed yet), so
it's the natural third game.
