# Task runner: https://github.com/casey/just  (install: `winget install Casey.Just`)
# Every recipe is a plain cargo command, so you can also copy it and run it directly.

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Run everything CI runs.
check: fmt-check lint test deny

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

test:
    cargo test --workspace --locked

# Needs `cargo install cargo-deny --locked`.
deny:
    cargo deny check

# Privacy harnesses (egress + file access, crates/gmm-cli/tests/privacy.rs) against a release
# build. On Linux, install strace and set GMM_PRIVACY_STRACE=1 to trace every syscall too.
audit-privacy:
    cargo test -p gmm-cli --test privacy --release --locked -- --nocapture

# Desktop app (apps/desktop). Needs Node 24 with corepack (`corepack pnpm ...` if `pnpm`
# isn't on PATH). The TypeScript types in src/bindings.ts come from the Rust types:
# `just bindings` regenerates them.
app-dev:
    cd apps/desktop; corepack pnpm tauri dev

app-check:
    cd apps/desktop; corepack pnpm install --frozen-lockfile; corepack pnpm typecheck; corepack pnpm lint; corepack pnpm audit; corepack pnpm run check-licenses

bindings:
    $env:GMM_UPDATE_BINDINGS = "1"; cargo test -p gmm-desktop bindings
