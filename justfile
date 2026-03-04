# Imageflow development commands

# Run all integration tests with nextest
test:
    cargo nextest run -p imageflow_core --test integration

# Run a specific test by name filter
test-filter filter:
    cargo nextest run -p imageflow_core --test integration -E 'test({{filter}})'

# Run tests with checksum auto-update (accepts within tolerance)
test-update:
    UPDATE_CHECKSUMS=1 cargo nextest run -p imageflow_core --test integration

# Replace all checksum baselines with current output
test-replace:
    REPLACE_CHECKSUMS=1 cargo nextest run -p imageflow_core --test integration

# Build tests without running (compile check)
test-build:
    cargo nextest run -p imageflow_core --test integration --no-run

# List all integration tests
test-list:
    cargo nextest list -p imageflow_core --test integration

# Run tests with cargo test (fallback, no nextest)
test-cargo:
    cargo test -p imageflow_core --test integration

# Check the whole workspace
check:
    cargo check --workspace

# Format and lint
fmt:
    cargo fmt --all
    cargo clippy -p imageflow_core --tests -- -D warnings
