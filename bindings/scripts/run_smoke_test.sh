#!/bin/bash
# Runs the smoke test for a given language binding.

set -e

if [ -z "$1" ] || [ -z "$2" ]; then
    echo "Usage: $0 <language> <mode>"
    echo "Example: $0 ruby local"
    exit 1
fi

LANGUAGE=$1
BUILD_MODE=$2 # "local" or "docker"
ROOT_DIR=$(cd "$(dirname "$0")/../.." && pwd)
LANGUAGE_OUTPUT_DIR="$ROOT_DIR/bindings/imageflow-$LANGUAGE"
TEST_OUTPUT_FILE="$LANGUAGE_OUTPUT_DIR/smoke_test_output.txt"

echo "--- Running smoke test for $LANGUAGE bindings in $BUILD_MODE mode... ---"

run_test() {
    local TEST_COMMAND="$1"
    local VERIFY_COMMAND="$2"

    if [ "$BUILD_MODE" == "docker" ]; then
        local BINDING_GENERATOR_IMAGE_NAME="imageflow-binding-generator"
        echo "--- Building monolithic binding generator image (if needed)... ---"
        docker build -t "$BINDING_GENERATOR_IMAGE_NAME" -f "$ROOT_DIR/bindings/docker/Dockerfile" "$ROOT_DIR" > /dev/null

        echo "--- Running $LANGUAGE smoke test inside container... ---"
        docker run --rm \
            -v "$ROOT_DIR:/work" \
            -w "/work/bindings/imageflow-$LANGUAGE" \
            "$BINDING_GENERATOR_IMAGE_NAME" \
            bash -c "$TEST_COMMAND" | tee "$TEST_OUTPUT_FILE"

    elif [ "$BUILD_MODE" == "local" ]; then
        echo "--- Running $LANGUAGE smoke test locally... ---"
        (cd "$LANGUAGE_OUTPUT_DIR" && bash -c "$TEST_COMMAND") | tee "$TEST_OUTPUT_FILE"
    else
        echo "Invalid mode: $BUILD_MODE. Use 'docker' or 'local'."
        exit 1
    fi

    echo "--- Verifying $LANGUAGE smoke test output ---"
    cat "$TEST_OUTPUT_FILE"
    if ! grep -qE "$VERIFY_COMMAND" "$TEST_OUTPUT_FILE"; then
        echo "$LANGUAGE smoke test failed or did not complete successfully."
        exit 1
    fi
    echo "$LANGUAGE smoke test passed."
}

case "$LANGUAGE" in
    go)
        # Go bindings are managed manually. Skipping.
        echo "Skipping Go smoke test."
        exit 0
        ;;
    ruby)
        # Test context creation and the JSON send/receive loop via the version endpoint.
        TEST_COMMAND="bundle install --quiet && ruby -r ./lib/imageflow -e 'ctx = Imageflow::Context.new; response = ctx.send_json(\"v1/get_version_info\", nil); if response[\"data\"][\"abi_major\"] == 3; puts \"JSON API version check passed.\"; end'"
        VERIFY_COMMAND="JSON API version check passed."
        run_test "$TEST_COMMAND" "$VERIFY_COMMAND"
        ;;
    *)
        echo "Unsupported language for smoke test: $LANGUAGE"
        exit 1
        ;;
esac

echo "--- Smoke test for $LANGUAGE complete. ---"
