#!/bin/bash
# Universal script to generate language-specific models for Imageflow.
# Works locally or inside the monolithic Docker container.

set -e # Exit immediately on error

# --- Configuration & Argument Parsing ---
if [ -z "$1" ] || [ -z "$2" ]; then
    echo "Usage: $0 <language> <mode>" >&2
    echo "Example: $0 ruby local" >&2
    exit 1
fi

LANGUAGE=$1
BUILD_MODE=$2 # "local" or "docker"
ROOT_DIR=$(cd "$(dirname "$0")/../.." && pwd)
SCHEMA_PATH="$ROOT_DIR/imageflow_core/src/json/endpoints/openapi_schema_v1.json"
OUTPUT_DIR="$ROOT_DIR/bindings/imageflow-$LANGUAGE"

# The calling script is responsible for creating and cleaning the output directory.

# --- Language-Specific Generation ---

echo "Running openapi-generator-cli for '$LANGUAGE' (models only) in '$BUILD_MODE' mode..."

# Define the command arguments once
# We add `--global-property models` to only generate model files.
case "$LANGUAGE" in
  typescript)
    GENERATOR_ARGS="-i $SCHEMA_PATH -g typescript-fetch --global-property models,supportingFiles --reserved-words-mappings string=StringModel --additional-properties=npmName=@imageflow/client,npmVersion=0.1.0,typescriptThreePlus=true,supportsES6=true -o $OUTPUT_DIR"
    ;;
  ruby)
    GENERATOR_ARGS="-i $SCHEMA_PATH -g ruby --global-property models,supportingFiles --additional-properties=gemName=imageflow,moduleName=Imageflow,gemVersion=0.1.0,modelPropertyNaming=original,useUnionTypes=true -o $OUTPUT_DIR"
    ;;
  *)
    echo "Error: Unsupported language '$LANGUAGE' for model generation." >&2
    echo "Go bindings are managed manually and not generated." >&2
    exit 1
    ;;
esac

# Execute the command based on the mode
if [ "$BUILD_MODE" == "docker" ]; then
    BINDING_GENERATOR_IMAGE_NAME="imageflow-binding-generator"
    # The generator is at /usr/local/lib/openapi-generator-cli.jar in the container
    docker run --rm \
        -v "$ROOT_DIR:/work" \
        -w "/work" \
        "$BINDING_GENERATOR_IMAGE_NAME" \
        java -jar /usr/local/lib/openapi-generator-cli.jar generate $GENERATOR_ARGS
elif [ "$BUILD_MODE" == "local" ]; then
    echo "Ensuring NPM dependencies are installed..."
    (cd "$ROOT_DIR/bindings" && npm install --quiet)
    echo "Running NPM-based generator..."
    (cd "$ROOT_DIR/bindings" && npx @openapitools/openapi-generator-cli generate $GENERATOR_ARGS)
else
    echo "Invalid mode: $BUILD_MODE. Use 'docker' or 'local'."
    exit 1
fi

echo "--- Model-only binding generation for '$LANGUAGE' complete! ---"
echo "Output available at: $OUTPUT_DIR"
