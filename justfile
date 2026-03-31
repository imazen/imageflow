# Imageflow development commands

# Run all integration tests with nextest
test:
    cargo nextest run -p imageflow_core --features "zen-pipeline,c-codecs" --test integration

# Run a specific test by name filter
test-filter filter:
    cargo nextest run -p imageflow_core --features "zen-pipeline,c-codecs" --test integration -E 'test({{filter}})'

# Run tests with checksum auto-update (accepts within tolerance)
test-update:
    UPDATE_CHECKSUMS=1 cargo nextest run -p imageflow_core --features "zen-pipeline,c-codecs" --test integration

# Alias for test-update (there is no separate "replace" mode)
test-replace:
    UPDATE_CHECKSUMS=1 cargo nextest run -p imageflow_core --features "zen-pipeline,c-codecs" --test integration

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

# Backfill diff stats on auto-accepted entries (downloads images from S3)
backfill-diffs:
    cargo test -p imageflow_core --test integration backfill_diff_stats -- --ignored --nocapture

# Download checksums from latest CI run for a specific platform
accept-ci platform branch="ci-speedup":
    #!/usr/bin/env bash
    set -euo pipefail
    RUN_ID=$(gh run list --branch "{{branch}}" --limit 1 --json databaseId --jq '.[0].databaseId')
    if [ -z "$RUN_ID" ]; then
        echo "No CI runs found for branch {{branch}}"
        exit 1
    fi
    echo "Downloading checksums-{{platform}} from run $RUN_ID..."
    TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TMPDIR"' EXIT
    gh run download "$RUN_ID" --name "checksums-{{platform}}" --dir "$TMPDIR"
    VISUALS=imageflow_core/tests/integration/visuals
    for f in "$TMPDIR"/*.checksums; do
        base=$(basename "$f")
        if [ -f "$VISUALS/$base" ]; then
            cp "$f" "$VISUALS/$base"
            echo "Updated: $base"
        fi
    done
    echo "Done. Review changes with: git diff $VISUALS/"

# Check the whole workspace
check:
    cargo check --workspace

# Format and lint
fmt:
    cargo fmt --all
    cargo clippy -p imageflow_core --tests -- -D warnings

# Build fuzz targets with C decoder coverage instrumentation
fuzz-build:
    cd fuzz && CC=clang CXX=clang++ \
        CFLAGS="-fsanitize-coverage=inline-8bit-counters,indirect-calls,trace-cmp,pc-table" \
        CXXFLAGS="-fsanitize-coverage=inline-8bit-counters,indirect-calls,trace-cmp,pc-table" \
        cargo +nightly fuzz build

# Run all fuzz targets for a given duration (default 120s each)
fuzz duration="120":
    #!/usr/bin/env bash
    set -euo pipefail
    export CC=clang CXX=clang++
    export CFLAGS="-fsanitize-coverage=inline-8bit-counters,indirect-calls,trace-cmp,pc-table"
    export CXXFLAGS="$CFLAGS"
    cd fuzz
    for target in fuzz_decode fuzz_transcode fuzz_riapi fuzz_json; do
        echo "=== Running $target for {{duration}}s ==="
        dict=imageflow.dict
        if [ "$target" = "fuzz_riapi" ]; then dict=riapi.dict; fi
        if [ "$target" = "fuzz_json" ]; then dict=json.dict; fi
        cargo +nightly fuzz run "$target" -- \
            -fork=4 -dict="$dict" -max_total_time={{duration}} \
            -rss_limit_mb=2048 -max_len=4096 \
            2>&1 | tee /tmp/fuzz-${target}.log
        echo ""
    done
    echo "=== All targets complete ==="
    grep "oom/timeout/crash:" /tmp/fuzz-fuzz_*.log | tail -4

# Run a single fuzz target (e.g., just fuzz-one fuzz_decode 300)
fuzz-one target duration="120":
    #!/usr/bin/env bash
    set -euo pipefail
    export CC=clang CXX=clang++
    export CFLAGS="-fsanitize-coverage=inline-8bit-counters,indirect-calls,trace-cmp,pc-table"
    export CXXFLAGS="$CFLAGS"
    cd fuzz
    dict=imageflow.dict
    if [ "{{target}}" = "fuzz_riapi" ]; then dict=riapi.dict; fi
    if [ "{{target}}" = "fuzz_json" ]; then dict=json.dict; fi
    cargo +nightly fuzz run "{{target}}" -- \
        -fork=4 -dict="$dict" -max_total_time={{duration}} \
        -rss_limit_mb=2048 -max_len=4096
