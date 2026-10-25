<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/brand/gmm-lockup-dark.png">
  <img src="docs/brand/gmm-lockup-light.png" alt="Gacha Mod Manager, powered by GameBanana" width="520">
</picture>

# Gacha Mod Manager

A desktop app for finding, downloading and organizing GameBanana mods for gacha games, using
[XXMI](https://github.com/SpectrumQT/XXMI-Launcher) as the mod loader.

**Status:** early development. The backend is complete and works from the `gmm` command
line: browse GameBanana, check your setup (`gmm status`), install mods into XXMI's Mods folder
with a safety scan (`gmm install`), switch them on and off, uninstall them to the Recycle Bin,
spot conflicts, show disk use, and start a game with mods through XXMI (`gmm launch`). Zip,
7z and RAR mods are supported. The app
window has the designed sidebar shell with the startup check, the Dashboard (Launch), the
Mods Library, Downloads (the install queue) and the install confirmation; the Mods Store and
the remaining screens come next.
Progress and plans: [docs/README.md](docs/README.md).

First games: Zenless Zone Zero and Wuthering Waves.

## Try it (developers)

Needs Rust (see `rust-toolchain.toml`) on Windows.

```bash
cargo run -p gmm-cli -- status
```

```bash
cargo run -p gmm-cli -- catalog list --game zzz --sort downloads
```

RAR mods go through a separate helper program; build it once next to `gmm` (the app's
`tauri dev` and `tauri build` do this themselves):

```bash
cargo build -p gmm-extract
```

The app window needs Node 24 too (pnpm comes through corepack):

```bash
cd apps/desktop && corepack pnpm install && corepack pnpm tauri dev
```

## Mods load through XXMI

This app downloads and manages mods. It doesn't load them. Mods only take effect when the game is
started through XXMI. Install XXMI from its
[official releases](https://github.com/SpectrumQT/XXMI-Launcher/releases) and set it up for your
game. The app detects your setup and walks you through any missing steps.

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
