#!/bin/bash
# This script checks if the OpenAPI schema has changed since the last run.

set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)

SCHEMA_FILE="$ROOT_DIR/imageflow_core/src/json/endpoints/openapi_schema_v1.json"
STATE_FILE="$ROOT_DIR/bindings/.schema_state"

# Function to get the SHA256 hash of the current schema file
get_current_hash() {
    sha256sum "$SCHEMA_FILE" | awk '{ print $1 }'
}

# Function to get the last stored hash
get_stored_hash() {
    if [ -f "$STATE_FILE" ]; then
        cat "$STATE_FILE"
    else
        echo ""
    fi
}

# --- Main Logic ---

CURRENT_HASH=$(get_current_hash)
STORED_HASH=$(get_stored_hash)

if [ "$CURRENT_HASH" == "$STORED_HASH" ]; then
    echo "Schema has not changed."
    exit 0 # Exit code 0 means no changes
else
    echo "Schema has changed. Stored hash: $STORED_HASH, New hash: $CURRENT_HASH"
    echo "An AI agent should be triggered to update the bindings."
    # Update the state file with the new hash
    echo "$CURRENT_HASH" > "$STATE_FILE"
    exit 1 # Exit code 1 means changes were detected
fi
