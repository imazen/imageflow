#!/bin/bash
# Builds the native library (libimageflow.so) and generates the OpenAPI schema.

set -e

ROOT_DIR=$(cd "$(dirname "$0")/../.." && pwd)
BUILD_MODE=${1:-"docker"} # Default to docker mode
TARGET_DIR="$ROOT_DIR/bindings/bin/native"

BUILDER_IMAGE_NAME="imageflow-builder"
CACHE_DIR="$ROOT_DIR/.cache/cargo_home"
mkdir -p "$CACHE_DIR"
mkdir -p "$TARGET_DIR"

run_cargo_command() {
    local CARGO_COMMAND="$1"
    # Add the target-dir to the command
    local FULL_CARGO_COMMAND="$CARGO_COMMAND --target-dir $TARGET_DIR"

    if [ "$BUILD_MODE" == "docker" ]; then
        echo "--- Building native library builder image (if needed)... ---"
        docker build -t "$BUILDER_IMAGE_NAME" "$ROOT_DIR/docker/builder" > /dev/null

        echo "--- Running cargo command in Docker: '$FULL_CARGO_COMMAND' ---"
        # Adjust path for Docker volume mount
        local DOCKER_TARGET_DIR="/work/bindings/bin/native"
        local DOCKER_CARGO_COMMAND="$CARGO_COMMAND --target-dir $DOCKER_TARGET_DIR"

        docker run --rm \
            -u "$(id -u):$(id -g)" \
            -v "$ROOT_DIR:/work" \
            -v "$CACHE_DIR:/cargo" \
            -e "CARGO_HOME=/cargo" \
            "$BUILDER_IMAGE_NAME" \
            bash -c "$DOCKER_CARGO_COMMAND"
    elif [ "$BUILD_MODE" == "local" ]; then
        echo "--- Running cargo command locally: '$FULL_CARGO_COMMAND' ---"
        (cd "$ROOT_DIR" && $FULL_CARGO_COMMAND)
    else
        echo "Invalid mode: $BUILD_MODE. Use 'docker' or 'local'."
        exit 1
    fi
}

echo "--- Building native library (libimageflow.so)... ---"
run_cargo_command "cargo build --release --package imageflow_abi"

echo "--- Generating OpenAPI schema... ---"
run_cargo_command "cargo test --features schema-export --test schema"

echo "--- Native build and schema generation complete. ---"
