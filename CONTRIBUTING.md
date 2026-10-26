# Contributing

Thanks for helping. Bug reports are welcome as
[GitHub issues](https://github.com/clorece/gacha-mod-manager/issues) (the app's **Report a
Bug** page fills one in for you). For a security problem, use the repository's private
vulnerability reporting (Security tab) instead of a public issue.

## Ground rules

- **Write it yourself.** Don't copy or port code, text or assets from other projects,
  especially other mod managers or XXMI. Work from the docs in `docs/`. If you need a new fact
  about a game or XXMI, get it by observing behaviour (files, configs), not by reading their
  code, and add it to `docs/game-adapters.md`. See `docs/licensing.md`.
- **Security rules are not optional.** Read `docs/security.md`. In short:
  - Filesystem access only through `gmm-fsguard`, network only through `gmm-net`. Clippy
    enforces both.
  - Processes are started in two places only: XXMI Launcher (fixed arguments) and GMM's own
    RAR helper. CI checks this.
  - Never read secrets: XXMI's `github_token`, `proxy` and signatures, Steam login files,
    launcher login caches, game logs. Parse config files into structs that declare only the
    fields GMM needs.
  - PRs that add a network host, a filesystem root, a process launch, a Cargo feature, or a
    dependency with a build script need the `security-review` label.
- **No telemetry.** Don't add analytics, crash uploaders or "phone home" features.
- **Docs describe what exists.** If you change behaviour a doc describes, update the doc in the
  same PR.

## Setting up

See [Build from source](README.md#build-from-source) in the README for the tools (Rust through
rustup, Node.js 24, a C/C++ compiler, and the Linux system libraries). The
[`justfile`](justfile) has shortcuts for everything below (`just check`, `just app-check`);
each recipe is a plain command you can also run directly.

## Before you open a PR

Run what CI runs:

```bash
cargo fmt --all -- --check
```

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings
```

```bash
cargo test --workspace --locked
```

```bash
cargo deny check
```

If you touched the app's frontend (`apps/desktop/src`), also run, in `apps/desktop`:

```bash
corepack pnpm typecheck && corepack pnpm lint && corepack pnpm test && corepack pnpm run check-licenses
```

If you changed a Rust type the frontend uses, regenerate `apps/desktop/src/bindings.ts` with
`just bindings`. Include `Cargo.lock` and `pnpm-lock.yaml` in your commit when they changed.

## Sign-off

Every commit must be signed off (`git commit -s`), certifying the
[Developer Certificate of Origin](https://developercertificate.org/).

## Releasing (maintainers)

1. Set the version in `Cargo.toml` (`[workspace.package]`, e.g. `0.0.2-beta`),
   `apps/desktop/package.json` (the same) and `apps/desktop/src-tauri/tauri.conf.json` (the
   numbers only, e.g. `0.0.2`, because MSI versions can't carry `-beta`). Run
   `cargo update --workspace` to refresh `Cargo.lock`.
   `node scripts/check-release-version.mjs v0.0.2-beta` checks they agree.
2. Write the release notes in `docs/releases/v<version>.md` and go through the release
   checklist in `docs/licensing.md`.
3. Commit, then push a tag `v<version>`. The Release workflow builds the MSI and its SHA-256
   and creates a **draft** GitHub release with those notes (a pre-release when the version has
   `-beta`). Code signing runs automatically once the `SIGNPATH_ORGANIZATION_ID` variable and
   the `SIGNPATH_API_TOKEN` secret exist in the repository settings.
4. Check the draft and publish it.

## License of contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project by you, as defined in the Apache-2.0 license, shall be dual licensed under MIT OR
Apache-2.0, without any additional terms or conditions.
