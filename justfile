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
