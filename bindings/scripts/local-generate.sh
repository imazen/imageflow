#!/bin/bash
# local-generate.sh
# This script provides a local, iterative workflow for generating language bindings.

# Stop on any errors
set -e

# Add cargo to the path
export PATH="$HOME/.cargo/bin:$PATH"

# Get the script's directory to determine the project root
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
PROJECT_ROOT=$(realpath "$SCRIPT_DIR/..")

# --- Configuration ---
LANG="node" # The language to generate for
SCHEMA_FILE_RELATIVE="imageflow_core/src/json/endpoints/openapi_schema_v1.json"
SCHEMA_FILE_ABSOLUTE="$PROJECT_ROOT/$SCHEMA_FILE_RELATIVE"
OUTPUT_RELATIVE="bindings/imageflow-$LANG"
OUTPUT_ABSOLUTE="$PROJECT_ROOT/$OUTPUT_RELATIVE"
# The native path is required by the workflow, but not used by this specific generator
NATIVE_BINARIES_PATH="$PROJECT_ROOT/artifacts/native_binaries"
DOCKERFILE_PATH="$PROJECT_ROOT/bindings/docker/$LANG"
DOCKER_IMAGE_TAG="imageflow/binding-generator-$LANG:latest"

# --- 1. Generate OpenAPI Schema ---
echo "Generating OpenAPI schema..."
# This command is taken directly from the schema-update.yml workflow
(cd "$PROJECT_ROOT" && cargo test --features schema-export --test schema)
echo "Schema generation complete."

# --- 2. Build Native Binaries (Placeholder) ---
# In a real-world scenario for other languages, you would build the native binaries here.
# For now, we'll just ensure the directory exists, as the generator script expects the path argument.
echo "Ensuring native binaries directory exists (placeholder)..."
mkdir -p "$NATIVE_BINARIES_PATH"

# --- 3. Build the Generator Docker Image ---
echo "Building Docker generator image for '$LANG' from $DOCKERFILE_PATH..."
docker build -t "$DOCKER_IMAGE_TAG" "$DOCKERFILE_PATH"
echo "Docker image '$DOCKER_IMAGE_TAG' built successfully."

# --- 4. Run the Generator ---
echo "Running the binding generator for '$LANG'..."
echo "  Schema: $SCHEMA_FILE_RELATIVE"
echo "  Output: $OUTPUT_RELATIVE"

# We mount the entire project root to /work inside the container.
# This allows the script to access the schema and write to the output directory.
docker run --rm -v "$PROJECT_ROOT:/work" \
    "$DOCKER_IMAGE_TAG" \
    --schema-path "/work/$SCHEMA_FILE_RELATIVE" \
    --output-path "/work/$OUTPUT_RELATIVE" \
    --native-path "/work/artifacts/native_binaries"

echo "Binding generation for '$LANG' complete!"
echo "Generated files are in: $OUTPUT_ABSOLUTE"
