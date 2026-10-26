# Brand assets

The final GMM mark and lockups, provided by the project owner (made with Claude Design,
2026-10-24). Original work for this project, under the repository's license.

| File | Use |
|---|---|
| `gmm-logo.svg` | The mark: a glossy gacha capsule with a candy-yellow plus. Vector, transparent, shadow built in, viewBox `-16 -16 272 272`. Source of the app icons (`apps/desktop/src-tauri/icons/`, generated with `pnpm tauri icon`) and of the in-app logo (`apps/desktop/src/assets/gmm-logo.svg`). Minimum size about 24 px |
| `gmm-lockup-light.png` | Full lockup ("Gacha Mod Manager" + mark + "Powered by GameBanana" pill) for light backgrounds, 2968×696 |
| `gmm-lockup-dark.png` | The same for dark backgrounds |

In the app, build the lockup live (text + SVG) so it stays crisp:

- Title: Nunito 900, letter-spacing −0.012em, vertical gradient fill: light `#6FD3FF` →
  `#3A6BFF`, dark `#B5F5FF` → `#5B9BFF`.
- Pill: Nunito 800, a 7 px `#FFB020` dot, background light `#EEF4FF` / dark
  `rgba(127,219,255,.12)`, text light `#3A5A9A` / dark `#B9D2F5`.
- On dark surfaces the mark gets `drop-shadow(0 0 18px rgba(92,200,255,.35))`.
- Hover: `scale(1.07) rotate(-5deg)` with a springy curve, off under reduced motion.

"GameBanana" is plain text; never use GameBanana's own logo without their permission. The full
token list for both themes is in `apps/desktop/src/styles.css`.
