<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/brand/gmm-lockup-dark.png">
  <img src="docs/brand/gmm-lockup-light.png" alt="Gacha Mod Manager, powered by GameBanana" width="520">
</picture>

# Gacha Mod Manager

A desktop app for finding, downloading and organizing [GameBanana](https://gamebanana.com)
mods for gacha games, using [XXMI](https://github.com/SpectrumQT/XXMI-Launcher) as the mod
loader. Supported games: **Zenless Zone Zero** and **Wuthering Waves**.

**Status:** 0.0.1 beta, the first public test release. Windows installer (unsigned for now);
Linux builds from source and is experimental.

- **Mods Store**: browse and search GameBanana by category and character, with a full mod
  page (previews, description, files, updates, credits, license).
- **Safe installs**: every download is checked before anything reaches XXMI's Mods folder.
  Zip, 7z and RAR are supported.
- **Mods Library**: switch mods on and off (your in-game toggles are kept), versions, updates
  and roll back, key bindings, profiles, conflicts, adopting mods you installed by hand,
  installing from a local archive.
- **Dashboard**: launch a game with mods through XXMI, with setup checks that tell you what to
  fix.
- **No account, no telemetry.** GMM talks to GameBanana (and to GitHub's releases list, to
  check for a newer GMM) and nothing else.

## How it works

GMM downloads and manages mods. It doesn't load them: **XXMI does**, and only when the game is
started through XXMI. GMM puts mods in XXMI's `Mods` folder for each game (its own go into
`Mods/GMM/`), switches them on and off, and starts XXMI for you. It never changes game files
and never bundles or modifies XXMI.

## Requirements

- **Windows 10 or 11, 64-bit.** Linux: see [Linux (experimental)](#linux-experimental).
- **[XXMI Launcher](https://github.com/SpectrumQT/XXMI-Launcher/releases)**, installed and set
  up for each game you want to mod (the importer for that game installed, the game folder
  set). GMM checks this and walks you through anything missing.
- **Zenless Zone Zero** and/or **Wuthering Waves**, installed through Steam, HoYoPlay, Epic
  Games, or anywhere else (you can add a folder by hand).
- An internet connection, for GameBanana.

## Install

### Windows

1. **Download** `GMM-<version>-x64.msi` and `SHA256SUMS.txt` from the
   [Releases page](https://github.com/clorece/gacha-mod-manager/releases).
2. **Check the download** (recommended, since the installer isn't signed yet). In PowerShell,
   in the folder you downloaded to:

   ```powershell
   Get-FileHash .\GMM-0.0.1-beta-x64.msi -Algorithm SHA256
   ```

   The hash it prints must match the one in `SHA256SUMS.txt`. If it doesn't, delete the file
   and don't run it.
3. **Run the installer.** Because it's unsigned, Windows will likely stop it first; see
   [Unsigned installer warnings](#unsigned-installer-warnings) below. The installer asks for
   administrator permission and installs Microsoft Edge WebView2 if it's missing (Windows 11
   already has it).
4. **Start Gacha Mod Manager** from the Start menu and follow [First run](#first-run).

To **uninstall**, use Settings → Apps → Installed apps → Gacha Mod Manager. Your mods stay in
XXMI's Mods folders; GMM's own data stays in `%APPDATA%\GMM` and `%LOCALAPPDATA%\GMM`, which
you can delete by hand.

#### Unsigned installer warnings

Code signing is being set up: free code signing provided by
[SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org). Until a signed release is out, Windows treats
GMM like any program from an unknown publisher. There are two different
protections you may meet:

**SmartScreen: "Windows protected your PC"**

This is the common one. Click **More info**, check that the file name is the one you
downloaded, then click **Run anyway**. You can also avoid the prompt before you run the file:
right-click the `.msi` → **Properties** → tick **Unblock** at the bottom of the General tab →
**OK**.

**Smart App Control: "Part of this app has been blocked"**

Smart App Control is stricter: it blocks unsigned programs outright, with no "Run anyway"
button and no way to allow a single app. It's only on for some PCs (mostly fresh installs of
Windows 11). It blocks GMM's programs every time they start, not just the installer, so using
an unsigned GMM means switching it off:

1. Open **Windows Security** → **App & browser control** → **Smart App Control settings**.
2. If it says **On** (or **Evaluation** and GMM is being blocked), choose **Off**.

Smart App Control is a real protection, so decide whether that trade-off is right for you.
On Windows 11 with the April 2026 update (KB5083769) or later you can turn it back on from the
same page. On **older builds, turning it off is permanent** until Windows is reset or
reinstalled. If you'd rather not turn it off, wait for a signed release.

### Linux (experimental)

There's no Linux package yet, so you build GMM from source. It builds and its tests pass on
Ubuntu in CI, but it hasn't been tried against a real Linux gaming setup. Expect rough edges,
and please [report](https://github.com/clorece/gacha-mod-manager/issues) what you find.

What to expect, since the games and XXMI are Windows programs running through Proton or Wine:

- **XXMI isn't found automatically.** Point GMM at the XXMI Launcher folder inside your Wine
  or Proton prefix (Settings → XXMI folder). It's usually
  `<prefix>/drive_c/users/<user>/AppData/Roaming/XXMI Launcher`.
- **Games:** Steam libraries under `~/.steam/steam` are detected. HoYoPlay and Epic installs
  aren't; add the game folder by hand (Settings → Games).
- **The setup check will likely say "game folder in XXMI is wrong"**, because XXMI stores the
  folder as a Windows path (`Z:\...`) that GMM can't match. Mods still install into the right
  Mods folder.
- **Launch** starts `XXMI Launcher.exe` directly, which only works if your system runs `.exe`
  files through Wine. Otherwise start XXMI the way you normally do; mods installed with GMM
  load all the same.

To build and run it:

1. Install the system libraries Tauri needs, plus a C/C++ compiler (for SQLite and the RAR
   reader).

   Debian / Ubuntu (tested in CI):

   ```bash
   sudo apt install build-essential curl file libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libxdo-dev libssl-dev
   ```

   Fedora (from Tauri's prerequisites, untested here):

   ```bash
   sudo dnf install gcc-c++ curl file webkit2gtk4.1-devel gtk3-devel librsvg2-devel libxdo-devel openssl-devel
   ```

   Arch (from Tauri's prerequisites, untested here):

   ```bash
   sudo pacman -S --needed base-devel curl file webkit2gtk-4.1 gtk3 librsvg xdotool openssl
   ```

2. Install **Rust** with [rustup](https://rustup.rs) (the repo pins the version, and rustup
   fetches it on first build) and **Node.js 24** (from [nodejs.org](https://nodejs.org) or
   your version manager).
3. Build it:

   ```bash
   git clone https://github.com/clorece/gacha-mod-manager.git
   ```

   ```bash
   cd gacha-mod-manager/apps/desktop && corepack pnpm install && corepack pnpm tauri build
   ```

4. Run it. The app and its RAR reader (`gmm-extract`) must stay side by side:

   ```bash
   ../../target/release/gmm-desktop
   ```

## First run

1. **Preferences:** pick a theme and whether to show sensitive (NSFW) mods. You can change both
   later in Settings.
2. **Startup check:** GMM looks for your games and XXMI and checks for newer mods.
3. **Dashboard:** each game shows **Ready** or what's missing. A setup card walks you through
   fixing it in XXMI (installing the importer, setting the game folder), then **Check again**.
4. **Mods Store:** find a mod, press **Download**, pick the files you want and confirm. The
   install runs on the **Downloads** page.
5. **Play:** open the game's banner on the Dashboard and press **Launch**. Mods only load when
   the game starts through XXMI.

The app's **Documentation** page has short guides on all of this, including fixing setup
problems and conflicts. The same guides are in [docs/guides](docs/guides/).

## Build from source

You need the same tools on every platform: **Rust** through [rustup](https://rustup.rs) (the
version is pinned in `rust-toolchain.toml`), **Node.js 24** (pnpm comes through corepack) and a
C/C++ compiler. On Windows that's the
[Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
"Desktop development with C++", which Rust's Windows toolchain needs anyway. On Linux, see the
packages above.

Run the app in development mode:

```bash
cd apps/desktop && corepack pnpm install && corepack pnpm tauri dev
```

Build a release binary (`target/release/gmm-desktop`, with `gmm-extract` beside it):

```bash
cd apps/desktop && corepack pnpm tauri build
```

Everything the app does also works from the `gmm` command line:

```bash
cargo run -p gmm-cli -- status
```

```bash
cargo run -p gmm-cli -- catalog list --game zzz --sort downloads
```

`cargo run -p gmm-cli -- --help` lists every command. RAR mods need the helper built once
next to `gmm` (`tauri dev` and `tauri build` do this themselves):

```bash
cargo build -p gmm-extract
```

To contribute, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Docs

| Doc | What it covers |
|---|---|
| [docs/guides/](docs/guides/) | Player guides, also shown on the app's Documentation page |
| [docs/architecture.md](docs/architecture.md) | Crates, data model, install and launch flows, app folders |
| [docs/game-adapters.md](docs/game-adapters.md) | Game and XXMI detection, setup states, adding a game |
| [docs/security.md](docs/security.md) | Threat model, the rules the code follows, test harnesses |
| [docs/licensing.md](docs/licensing.md) | Licenses, dependencies, clean-room rules, release checklist |
| [docs/gamebanana-api.md](docs/gamebanana-api.md) | GameBanana's terms and limits, and how GMM stays within them |
| [docs/brand/](docs/brand/) | The GMM mark and lockups |

## Credits

Gacha Mod Manager is an open-source interface built on other people's work:

- **[GameBanana](https://gamebanana.com)**: the mod catalogue, its public API and the file
  hosting every mod comes from. GMM links back to each mod's page and never re-hosts files.
- **Mod creators**: every mod belongs to its author on GameBanana. GMM shows their names,
  credits and licenses as GameBanana lists them.
- **[XXMI Launcher](https://github.com/SpectrumQT/XXMI-Launcher)** by SpectrumQT, with its
  importers (ZZMI, WWMI and others): the mod loader. GMM detects it, reads a few of its settings
  and starts it; it never bundles or modifies it.
- **[3Dmigoto](https://github.com/bo3b/3Dmigoto)**, which XXMI builds on: GMM reads its `.ini`
  format to find conflicts and key bindings.
- **[UnRAR](https://www.rarlab.com/rar_add.htm)** by Alexander Roshal (RARLAB): opens RAR mods,
  under its own freeware license (see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)).
- **[Nunito](https://github.com/googlefonts/nunito)** by the Nunito Project Authors: the
  interface font (SIL Open Font License 1.1).
- **Game banner art**: official key art of Zenless Zone Zero (© COGNOSPHERE / HoYoverse) and
  Wuthering Waves (© Kuro Games), cropped for the game banners. Not covered by GMM's license;
  see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
- **Open-source libraries**, among them [Tauri](https://tauri.app), [React](https://react.dev),
  [DOMPurify](https://github.com/cure53/DOMPurify), [TanStack Virtual](https://tanstack.com/virtual),
  [SQLx](https://github.com/launchbadge/sqlx),
  [reqwest](https://github.com/seanmonstar/reqwest), [rustls](https://github.com/rustls/rustls),
  [zip](https://github.com/zip-rs/zip2) and [sevenz-rust2](https://github.com/hasenbanck/sevenz-rust).
  The full list with licenses comes with each release.

If you are the original owner of credited work or assets and would like the credit adjusted
or the assets removed from the project, please contact me through
[GitHub](https://github.com/clorece/gacha-mod-manager/issues).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.

## Disclaimer

Not affiliated with or endorsed by HoYoverse, Kuro Games, GameBanana or the XXMI project. All
trademarks belong to their respective owners. Modding may violate a game's Terms of Service. Use
at your own risk.
