# Gacha Mod Manager (working name)

A desktop app for finding, downloading and organizing GameBanana mods for gacha games, using
[XXMI](https://github.com/SpectrumQT/XXMI-Launcher) as the mod loader.

**Status:** early development, command line only. The backend can browse GameBanana, download
and safely extract mods, find your games and XXMI, check your mod setup (`gmm status`), remember
your settings (`gmm config`), and install mods into XXMI's Mods folder (`gmm install`).
It can also switch mods on and off, uninstall them (to the Recycle Bin), spot mods that
conflict (`gmm conflicts`), and show disk use (`gmm storage`). Launching through XXMI is next.
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
