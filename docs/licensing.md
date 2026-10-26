# Licensing & provenance

GMM is an original work, licensed **MIT OR Apache-2.0**, and written from scratch. This page
says how it stays that way. It isn't legal advice.

## Related projects

| Project | License | How GMM relates to it |
|---|---|---|
| deadlock-mod-manager | GPL-3.0 | Prior art: its feature list and README were read. No code, assets or text used |
| XXMI Launcher, ZZMI, WWMI, 3Dmigoto | GPL-3.0 | Separate programs the user installs. GMM reads a few config fields and starts XXMI with command-line arguments. Never bundled, linked or loaded into GMM's process |
| GameBanana mods | Per mod | Never re-hosted. Downloaded from GameBanana; credits and license shown on each mod page |

Starting a separate program with command-line arguments and exchanging data files is not
combining with it, so XXMI's GPL doesn't extend to GMM.

## Clean-room rules

1. **No GPL source open while writing GMM code**, and don't paste it into AI tools. Work from
   our own docs ([architecture](architecture.md), [game adapters](game-adapters.md)).
2. **New facts come from behaviour, not code**: compare files before and after, read configs,
   then write the fact into `game-adapters.md` and implement from that.
3. **Game settings stay with XXMI.** GMM never patches game files.
4. **No assets from other projects.** Icons, logo and screenshots are our own. Mod previews load
   live from GameBanana.
5. **Own wording everywhere**: docs, UI text, README.

## Dependencies

`cargo-deny` and `scripts/check-npm-licenses.mjs` enforce an allowlist in CI: MIT, Apache-2.0,
BSD-2/3-Clause, ISC, Zlib, MPL-2.0, Unicode-3.0, CC0-1.0 (and 0BSD, BlueOak-1.0.0, MIT-0 for
npm). GPL, LGPL and AGPL fail the build. Exceptions, each with a reason in `deny.toml` or the
script:

- **UnRAR** (`crates/gmm-extract/vendor/`): RARLAB's freeware license allows using the source
  to extract RAR files but not to build a RAR compressor. GMM only extracts. It's GMM's one
  non-OSI component, so it lives only in the separate `gmm-extract` program, has its own
  license entry (`LicenseRef-UnRAR`), and its full license is in `THIRD-PARTY-NOTICES.md`.
  libarchive (BSD) was tried first but failed one of five real RAR mods; UnRAR 7.23 extracted
  all five.
- **`webpki-root-certs`**: Mozilla's CA list, under the permissive CDLA-Permissive-2.0.
- **Nunito** (`@fontsource-variable/nunito`): the UI font, under the SIL Open Font License 1.1,
  with its notice in `THIRD-PARTY-NOTICES.md`.

7z BZip2 support is left out because of its license.

## Assets

- **The GMM mark and lockups** (`docs/brand/`, the app icons) were made for this project by
  its owner (with Claude Design) and are original to it.
- **Game banner art** (`apps/desktop/src/assets/banners/`): official key art of Zenless Zone
  Zero (© COGNOSPHERE / HoYoverse) and Wuthering Waves (© Kuro Games), used only to identify
  each game. It's the one shipped item that isn't ours or openly licensed: it isn't covered by
  GMM's license, the README and `THIRD-PARTY-NOTICES.md` say so, and it will be removed at the
  owners' request.
- GameBanana's logo is never used, only its name as text.

## Names and trademarks

Game and site names are used only descriptively ("mods for Wuthering Waves"), never in GMM's
name or logo. The README and the app's About page carry the disclaimer.

## Provenance record

- Git history from the first commit shows independent authorship.
- SPDX headers in source files: `// SPDX-License-Identifier: MIT OR Apache-2.0`.
- Every commit is signed off (`git commit -s`, see `CONTRIBUTING.md`).
- Commits made with an AI assistant carry a `Co-Authored-By` line.
- Research notes about other projects stay out of the repo (`docs/research/` is git-ignored).

## Release checklist

- [ ] `cargo deny check` and the npm license check pass
- [ ] `THIRD-PARTY-NOTICES.md` current and shown in Settings → About
- [ ] README Credits current, with the contact line for credit changes or removal
- [ ] No request from GameBanana, HoYoverse or Kuro Games is outstanding
- [ ] GameBanana's Terms of Service re-read (they can change without notice)
