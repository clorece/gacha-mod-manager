# Contributing

Bug reports are welcome as [issues](https://github.com/clorece/gacha-mod-manager/issues) (the
app's **Report a Bug** page fills one in for you). Security problems: use private vulnerability
reporting in the repository's Security tab instead.

## Rules

- **Write it yourself.** Don't copy or port code, text or assets from other projects,
  especially other mod managers or XXMI. New facts about a game or XXMI come from observing
  files and configs, not from reading their code. See [docs/licensing.md](docs/licensing.md).
- **Follow the security rules** in [docs/security.md](docs/security.md):
  - files only through `gmm-fsguard`, network only through `gmm-net` (clippy enforces both);
  - start no new processes (only XXMI and GMM's RAR helper are allowed);
  - never read secrets: XXMI's token, proxy and signatures, Steam login files, launcher
    caches, game logs.
- **No telemetry**, analytics or crash uploaders.
- **Keep docs true.** If you change something a doc describes, update the doc in the same PR.

PRs that add a network host, a filesystem root, a process launch, a Cargo feature or a
dependency with a build script need the `security-review` label.

## Setup

Tools are listed under [Build from source](README.md#build-from-source) in the README.

## Before you open a PR

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

If you changed the frontend, in `apps/desktop`:

```bash
corepack pnpm typecheck && corepack pnpm lint && corepack pnpm test
```

Or run `just check` and `just app-check` (see the [`justfile`](justfile)). Commit
`Cargo.lock` and `pnpm-lock.yaml` when they change.

## Sign-off

Sign off every commit with `git commit -s` ([Developer Certificate of
Origin](https://developercertificate.org/)).

## Releasing (maintainers)

1. Set the version in `Cargo.toml` and `apps/desktop/package.json` (e.g. `0.0.2-beta`) and in
   `apps/desktop/src-tauri/tauri.conf.json` (numbers only, e.g. `0.0.2`). Run
   `cargo update --workspace`.
2. Write the notes in `docs/releases/v<version>.md`.
3. Commit, then push the tag `v<version>`. The Release workflow builds the MSI and creates a
   draft release (signed automatically once the `SIGNPATH_ORGANIZATION_ID` variable and
   `SIGNPATH_API_TOKEN` secret are set).
4. On the Releases page, open the draft with **Edit** and press **Publish release**.

## License of contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project by you, as defined in the Apache-2.0 license, shall be dual licensed under MIT OR
Apache-2.0, without any additional terms or conditions.
