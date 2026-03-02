# Imageflow development recipes

# Verify lockfile matches Cargo.toml (reproduces CI --locked check)
check-locked:
    cargo check --locked

# Format, then verify lockfile
pre-commit: fmt check-locked

# cargo fmt
fmt:
    cargo fmt

# Run clippy
clippy:
    cargo clippy --all-targets
