#!/bin/bash
# Master workflow script for building, generating, and testing language bindings.
# Supports 'local' and 'docker' execution modes.

set -e # Exit immediately on error

# --- Configuration & Argument Parsing ---
if [ -z "$1" ] || [ -z "$2" ]; then
    echo "Usage: $0 <language> <mode>" >&2
    echo "Example: $0 ruby local" >&2
    exit 1
fi

LANGUAGE=$1
MODE=$2 # 'local' or 'docker'
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)
LANGUAGE_OUTPUT_DIR="$ROOT_DIR/bindings/imageflow-$LANGUAGE"

# Determine the correct native library file based on OS
if [[ "$OSTYPE" == "darwin"* ]]; then
    NATIVE_LIB_FILENAME="libimageflow.dylib"
else
    NATIVE_LIB_FILENAME="libimageflow.so"
fi
NATIVE_LIB_PATH="$ROOT_DIR/bindings/bin/native/release/$NATIVE_LIB_FILENAME"

# --- Workflow Orchestration ---

# 1. Build Native Library & Schema
echo "--- Step 1: Building native library and schema in '$MODE' mode... ---"
"$SCRIPT_DIR/build_native.sh" "$MODE"

# 2. Clean and Prepare Output Directory
echo "--- Step 2: Cleaning and preparing output directory for $LANGUAGE... ---"
rm -rf "$LANGUAGE_OUTPUT_DIR"
mkdir -p "$LANGUAGE_OUTPUT_DIR"

# 3. Generate Language Bindings
echo "--- Step 3: Generating $LANGUAGE bindings in '$MODE' mode... ---"
"$SCRIPT_DIR/generate_binding.sh" "$LANGUAGE" "$MODE"

# 4. Prepare generated client by copying native library and manual overrides
echo "--- Step 4: Preparing $LANGUAGE client... ---"
if [ ! -f "$NATIVE_LIB_PATH" ]; then
    echo "Error: Native library not found at $NATIVE_LIB_PATH" >&2
    exit 1
fi

case "$LANGUAGE" in
    ruby)
        mkdir -p "$LANGUAGE_OUTPUT_DIR/lib"
        echo "-> Copying native library..."
        cp "$NATIVE_LIB_PATH" "$LANGUAGE_OUTPUT_DIR/lib/"
        echo "-> Copying manual Ruby files over generated ones..."
        cp "$ROOT_DIR/bindings/templates/ruby/lib/imageflow.rb" "$LANGUAGE_OUTPUT_DIR/lib/"
        cp "$ROOT_DIR/bindings/templates/ruby/lib/imageflow_ffi.rb" "$LANGUAGE_OUTPUT_DIR/lib/"
        ;;
    go)
        cp "$NATIVE_LIB_PATH" "$LANGUAGE_OUTPUT_DIR/"
        ;;
    *)
        echo "Warning: No native library copy rule for $LANGUAGE. Skipping."
        ;;
esac

# 5. Run Smoke Test
echo "--- Step 5: Running smoke test for $LANGUAGE in '$MODE' mode... ---"
"$SCRIPT_DIR/run_smoke_test.sh" "$LANGUAGE" "$MODE"

echo "--- Workflow for $LANGUAGE in $MODE mode completed successfully! ---"
