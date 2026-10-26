<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/brand/gmm-lockup-dark.png">
  <img src="docs/brand/gmm-lockup-light.png" alt="Gacha Mod Manager, powered by GameBanana" width="520">
</picture>

# Gacha Mod Manager

Find, download and organize [GameBanana](https://gamebanana.com) mods for **Zenless Zone
Zero** and **Wuthering Waves**. Mods are loaded by
[XXMI Launcher](https://github.com/SpectrumQT/XXMI-Launcher).

**Status:** 0.0.1 beta. Windows installer (unsigned for now); Linux is experimental.

- Browse and search GameBanana's mods, with previews, files and updates.
- Every download is checked before it reaches your Mods folder. Zip, 7z and RAR work.
- Switch mods on and off, keep versions, update and roll back, save profiles, spot conflicts.
- Launch your game with mods in one click.
- No account and no telemetry.

## Before you start

You need:

1. **Windows 10 or 11** (64-bit).
2. **[XXMI Launcher](https://github.com/SpectrumQT/XXMI-Launcher/releases)**, set up for your
   game. GMM finds it and tells you if anything is missing.
3. **Zenless Zone Zero** and/or **Wuthering Waves** (Steam, HoYoPlay, Epic, or any folder).

GMM downloads and manages mods; XXMI loads them. Mods only work when the game is started
through XXMI (GMM's **Launch** button does that for you).

## Install on Windows

1. Download `GMM-<version>-x64.msi` from the
   [Releases page](https://github.com/clorece/gacha-mod-manager/releases).
2. **Right-click the `.msi` → Properties.** At the bottom of the **General** tab, tick
   **Unblock**, then press **OK**.
3. Double-click the `.msi` to install, and allow the administrator prompt.
4. Start **Gacha Mod Manager** from the Start menu.

Step 2 is needed because the installer isn't code-signed yet. If you skip it, Windows may show
"Windows protected your PC": click **More info** → **Run anyway**.

<details>
<summary>Still blocked? ("Smart App Control blocked an app")</summary>

Smart App Control is a stricter Windows feature that blocks all unsigned apps, with no way to
allow just one. It's only on for some PCs. To use GMM before a signed release, turn it off in
**Windows Security** → **App & browser control** → **Smart App Control settings** → **Off**.

On older Windows 11 builds, turning it off is permanent until Windows is reset; since the April
2026 update (KB5083769) you can turn it back on from the same page. If you'd rather not, wait
for a signed release.

</details>

<details>
<summary>Check the download (optional)</summary>

Download `SHA256SUMS.txt` from the same release, then in PowerShell, in your Downloads folder:

```powershell
Get-FileHash .\GMM-0.0.1-beta-x64.msi -Algorithm SHA256
```

The result must match the line in `SHA256SUMS.txt`.

</details>

**Uninstall:** Settings → Apps → Installed apps → Gacha Mod Manager. Your mods stay in XXMI's
Mods folders.

## First run

1. Pick a theme and whether to show sensitive mods.
2. On the **Dashboard**, each game shows **Ready** or what to fix in XXMI. Follow the steps,
   then press **Check again**.
3. Open the **Mods Store**, find a mod and press **Download**. It installs on the
   **Downloads** page.
4. Back on the Dashboard, open your game and press **Launch**.

The app's **Documentation** page has short guides (also in [docs/guides](docs/guides/)).

## Linux (experimental)

There's no Linux package yet; you build it yourself. It builds in CI but hasn't been tested
with a real Linux game setup, so please
[report](https://github.com/clorece/gacha-mod-manager/issues) what you find.

1. Install the build tools. On Debian / Ubuntu:

   ```bash
   sudo apt install build-essential curl file libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libxdo-dev libssl-dev
   ```

   Other distros: the same packages as
   [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/#linux), plus a C++
   compiler.
2. Install [Rust](https://rustup.rs) and [Node.js 24](https://nodejs.org).
3. Build and run:

   ```bash
   git clone https://github.com/clorece/gacha-mod-manager.git
   ```

   ```bash
   cd gacha-mod-manager/apps/desktop && corepack pnpm install && corepack pnpm tauri build
   ```

   ```bash
   ../../target/release/gmm-desktop
   ```

4. In GMM's **Settings**, set the **XXMI folder** to the one inside your Wine or Proton prefix
   (usually `drive_c/users/<user>/AppData/Roaming/XXMI Launcher`), and add your game folder if
   it isn't found.

Known limits: the setup check may say "game folder in XXMI is wrong" (XXMI stores a Windows
path), and **Launch** only works if your system runs `.exe` files through Wine. Otherwise start
XXMI the way you usually do; mods installed by GMM still load.

## Build from source

You need [Rust](https://rustup.rs), [Node.js 24](https://nodejs.org) and a C++ compiler (on
Windows, the
[Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with
"Desktop development with C++").

Run the app:

```bash
cd apps/desktop && corepack pnpm install && corepack pnpm tauri dev
```

Use the command line (every app feature works there too):

```bash
cargo run -p gmm-cli -- --help
```

Contributing: see [CONTRIBUTING.md](CONTRIBUTING.md). Technical docs are in [docs/](docs/):
[architecture](docs/architecture.md), [game adapters](docs/game-adapters.md),
[security](docs/security.md), [licensing](docs/licensing.md),
[GameBanana API](docs/gamebanana-api.md).

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

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org) (being set up; current releases are unsigned).

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
