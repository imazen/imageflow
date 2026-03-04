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

# Alias for test-update (there is no separate "replace" mode)
test-replace:
    UPDATE_CHECKSUMS=1 cargo nextest run -p imageflow_core --test integration

# Build tests without running (compile check)
test-build:
    cargo nextest run -p imageflow_core --test integration --no-run

# List all integration tests
test-list:
    cargo nextest list -p imageflow_core --test integration

# Run tests with cargo test (fallback, no nextest)
test-cargo:
    cargo test -p imageflow_core --test integration

# Upload new reference images to S3 (skips already-uploaded via uploaded.log)
upload:
    UPLOAD_REFERENCES=1 cargo test -p imageflow_core --test integration sync_and_verify_uploads -- --ignored --nocapture

# Verify all reference images are in uploaded.log (no credentials needed)
verify-uploads:
    cargo test -p imageflow_core --test integration verify_upload_log -- --ignored --nocapture

# Check the whole workspace
check:
    cargo check --workspace

# Format and lint
fmt:
    cargo fmt --all
    cargo clippy -p imageflow_core --tests -- -D warnings
