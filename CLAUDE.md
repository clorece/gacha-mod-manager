# Working agreements for this repo

These apply to every session. Read `docs/README.md` first for where development stands.

## When a phase (or a roadmap section) is finished

Without being asked, before reporting the phase as done:

1. **`docs/05-roadmap.md`**: tick every finished item, mark the phase heading done with the
   date, move anything deferred to the phase that will do it, and make the next phase's
   checklist concrete enough to start from.
2. **`docs/README.md`**: update the "Where we are" section (what's built, what's next, what's
   waiting on the user).
3. **Every other doc the work touched**: architecture (crate list, flows, data model),
   game adapters (detection sources, states, verified facts), security (rules and which
   harnesses exist), licensing (new dependencies, exceptions). Remove `[verify]` markers that
   were resolved and say where the fact came from.
4. **Root `README.md`**: keep its status line current.
5. **`docs/research/phase0-observations.md`** (local only): append any new facts observed on
   the dev machine.

Docs must describe what exists, not what was planned. If code and docs disagree, fix the docs
in the same change.

## Commits

Never run `git commit` or `git push`. At a commit point, give a commit title and description
the user can paste. The user commits with `git commit -s` (DCO sign-off, see CONTRIBUTING.md).
Remind them to include `Cargo.lock` when it changed.

## Code rules (summary; details in docs/04-security.md and docs/06-licensing-and-provenance.md)

- Clean room: never copy or port code from deadlock-mod-manager or XXMI. Work from the specs in
  `docs/`, and get new facts by observing behaviour (files, configs), not by reading their code.
- Filesystem access only through `gmm-fsguard`; network only through `gmm-net`; process
  launching only for the XXMI launcher, with fixed arguments. Clippy enforces the first two.
- Never read secrets: XXMI's `github_token`/`proxy`/signatures, Steam login files, launcher
  login caches, game logs. Parse config files into structs that declare only the fields needed.
- Inspecting the user's machine is fine when it's read-only and targeted. List file names
  before reading contents, and extract only the fields needed.
- Before reporting work as done, run: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo deny check`.
- In this environment, new shells may need `$env:Path` refreshed to find `cargo`.
