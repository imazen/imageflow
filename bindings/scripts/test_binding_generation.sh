#!/bin/bash
# End-to-end test script for building the native library and generating language bindings.
# Uses a monolithic Docker image for a consistent and cacheable workflow.

# --- Configuration & Argument Parsing ---
set -e # Exit immediately if a command exits with a non-zero status.

if [ -z "$1" ]; then
    echo "Usage: $0 <language>"
    echo "Example: $0 ruby"
    exit 1
fi

LANGUAGE=$1
ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

# --- Docker Image Names ---
BUILDER_IMAGE_NAME="imageflow-builder"
BINDING_GENERATOR_IMAGE_NAME="imageflow-binding-generator" # Monolithic image

# --- Build Caching ---
CACHE_DIR="$ROOT_DIR/.cache/cargo_home"
mkdir -p "$CACHE_DIR"

# --- Stage 1: Build Native Library & Schema ---
echo "--- Building native library builder image... ---"
docker build -t "$BUILDER_IMAGE_NAME" "$ROOT_DIR/docker/builder"

echo "--- Building native library (libimageflow.so)... ---"
docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT_DIR:/work" \
    -v "$CACHE_DIR:/cargo" \
    -e "CARGO_HOME=/cargo" \
    "$BUILDER_IMAGE_NAME" \
    cargo build --release --package imageflow_abi

echo "--- Generating OpenAPI schema... ---"
docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT_DIR:/work" \
    -v "$CACHE_DIR:/cargo" \
    -e "CARGO_HOME=/cargo" \
    "$BUILDER_IMAGE_NAME" \
    cargo test --features schema-export --test schema

# --- Stage 2: Generate Language Bindings ---
echo "--- Building monolithic binding generator image... ---"
docker build -t "$BINDING_GENERATOR_IMAGE_NAME" -f "$ROOT_DIR/bindings/docker/Dockerfile" "$ROOT_DIR"

LANGUAGE_OUTPUT_DIR="$ROOT_DIR/bindings/imageflow-$LANGUAGE"

echo "--- Cleaning and preparing output directory for $LANGUAGE... ---"
rm -rf "$LANGUAGE_OUTPUT_DIR"
mkdir -p "$LANGUAGE_OUTPUT_DIR"

echo "--- Running binding generation for $LANGUAGE... ---"
# Run the generator, capturing the exit code to ensure logs are always displayed.
GENERATOR_EXIT_CODE=0
docker run --rm \
    -u "$(id -u):$(id -g)" \
    -v "$ROOT_DIR:/work" \
    -v "$LANGUAGE_OUTPUT_DIR:/output" \
    "$BINDING_GENERATOR_IMAGE_NAME" \
    bash -c "/work/scripts/generate_binding.sh '$LANGUAGE' /output" > "$LANGUAGE_OUTPUT_DIR/generator.log" 2> "$LANGUAGE_OUTPUT_DIR/generator.err" || GENERATOR_EXIT_CODE=$?

echo "--- Reviewing generator output for $LANGUAGE ---"
echo "[generator.log]"
cat "$LANGUAGE_OUTPUT_DIR/generator.log"
echo "[generator.err]"
cat "$LANGUAGE_OUTPUT_DIR/generator.err"

# Exit if the generator failed
if [ $GENERATOR_EXIT_CODE -ne 0 ]; then
    echo "Binding generation failed with exit code $GENERATOR_EXIT_CODE."
    exit $GENERATOR_EXIT_CODE
fi

# --- Stage 3: Run Smoke Test ---
echo "--- Running smoke test for $LANGUAGE bindings... ---"
case "$LANGUAGE" in
  ruby)
    # The Ruby smoke test requires 'bundler' and dependencies.
    # We can run this inside the generator container which has Ruby installed.
    echo "Running Ruby smoke test inside container..."
    TEST_OUTPUT_FILE="$LANGUAGE_OUTPUT_DIR/rspec.log"
    docker run --rm \
        -u "$(id -u):$(id -g)" \
        -v "$ROOT_DIR:/work" \
        -w "/work/bindings/imageflow-ruby" \
        "$BINDING_GENERATOR_IMAGE_NAME" \
        bash -c "bundle config set --local path 'vendor/bundle' && bundle install && bundle exec rspec" | tee "$TEST_OUTPUT_FILE"

    echo "--- Verifying Ruby smoke test output ---"
    cat "$TEST_OUTPUT_FILE"
    if ! grep -q "0 failures" "$TEST_OUTPUT_FILE"; then
        echo "RSpec tests failed or did not complete successfully."
        exit 1
    fi
    echo "RSpec tests passed."
    ;;
  typescript)


    echo "Running TypeScript smoke test inside container..."
    echo "Patching tsconfig.json to add ES2017 support..."
    docker run --rm \
        -u "$(id -u):$(id -g)" \
        -v "$ROOT_DIR:/work" \
        -w "/work/bindings/imageflow-typescript" \
        "$BINDING_GENERATOR_IMAGE_NAME" \
        bash -c "node -e \"const fs = require('fs'); const p = 'tsconfig.json'; const c = JSON.parse(fs.readFileSync(p)); c.compilerOptions.lib = [...(c.compilerOptions.lib || []), 'es2017', 'dom']; fs.writeFileSync(p, JSON.stringify(c, null, 2));\""

    echo "Running TypeScript smoke test inside container..."
    docker run --rm \
        -u "$(id -u):$(id -g)" \
        -v "$ROOT_DIR:/work" \
        -w "/work/bindings/imageflow-typescript" \
        "$BINDING_GENERATOR_IMAGE_NAME" \
        bash -c "npm install && npm run build"

    echo "--- Verifying TypeScript build artifacts ---"
    VERIFY_FILE="dist/apis/DefaultApi.js"
    docker run --rm \
        -u "$(id -u):$(id -g)" \
        -v "$ROOT_DIR:/work" \
        -w "/work/bindings/imageflow-typescript" \
        "$BINDING_GENERATOR_IMAGE_NAME" \
        bash -c "if [ ! -f \"$VERIFY_FILE\" ]; then echo 'Verification failed: $VERIFY_FILE not found.'; exit 1; else echo 'Verified: $VERIFY_FILE exists.'; fi"
    ;;
  go)
    echo "Running Go smoke test inside container..."
    # The generated Go client is a library, so we need a main package to test it.
    # Copy our simple smoke test into the generated directory.
    cp "$ROOT_DIR/bindings/go/test/smoke_test.go" "$LANGUAGE_OUTPUT_DIR/"

    TEST_OUTPUT_FILE="$LANGUAGE_OUTPUT_DIR/go_smoke.log"
    docker run --rm \
        -u "$(id -u):$(id -g)" \
        -v "$ROOT_DIR:/work" \
        -w "/work/bindings/imageflow-$LANGUAGE" \
        "$BINDING_GENERATOR_IMAGE_NAME" \
        bash -c "ls -laR && rm -rf test && echo '--- after rm ---' && ls -laR && go mod tidy && go test -v" | tee "$TEST_OUTPUT_FILE"

    echo "--- Verifying Go smoke test output ---"
    cat "$TEST_OUTPUT_FILE"
    if ! grep -q "PASS" "$TEST_OUTPUT_FILE"; then
        echo "Go smoke test failed or did not complete successfully."
        exit 1
    fi
    echo "Go smoke test passed."
    ;;
  *)
    echo "No smoke test configured for language '$LANGUAGE'."
    ;;
esac

echo "--- Workflow for '$LANGUAGE' completed successfully! ---"
