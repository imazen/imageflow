#!/bin/bash
# Reasoning: Set strict mode to exit on errors and ensure failures in pipelines are caught.
set -e # Exit on failure.

# ---------------------------
# Setup: determine directories
# ---------------------------
# Reasoning: Get the current script directory and calculate the corresponding pack directory.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(dirname "$SCRIPT_DIR")"

# Create a unique test directory name using current timestamp and random value.
TEST_DIR_REL="trash/test_pack_$(date +%s)_$RANDOM"
# Reasoning: Create a unique and isolated test directory for running pack.sh.
TEST_DIR="${SCRIPT_DIR}/${TEST_DIR_REL}"
echo "TEST_DIR: ${TEST_DIR}"

# ---------------------------
# Setup: cleanup function for test environment
# ---------------------------
# Reasoning: Ensure that we clean up the test directory even if the test fails.
cleanup() {
    # Reasoning: Capture exit code and print cleanup message.
    local exit_code=$?
    echo "Cleaning up test environment..."
    rm -rf "${TEST_DIR}"
    cd "$SAVE_DIR"
    if [ $exit_code -ne 0 ]; then
        echo "❌ Test failed with exit code: $exit_code"
    else
        echo "✅ Test completed successfully"
    fi
    exit $exit_code
}
trap cleanup 1 2 3 6

# ---------------------------
# Setup: create test environment and mock binaries
# ---------------------------
echo "Creating test environment in: ${TEST_DIR}"
mkdir -p "${TEST_DIR}/binaries"

# Reasoning: Create minimal mock files that will be used by pack.sh.
touch "${TEST_DIR}/binaries/imageflow.dll"
touch "${TEST_DIR}/binaries/imageflow_tool.exe"

# ---------------------------
# Setup: export environment variables used by pack.sh
# ---------------------------
# Reasoning: The pack.sh script expects these environment variables.
export REL_BINARIES_DIR="ci/pack_nuget/tests/${TEST_DIR_REL}/binaries/"
export REL_NUGET_OUTPUT_DIR="ci/pack_nuget/tests/${TEST_DIR_REL}/nuget/"
export CI_TAG="v0.9-rc1-1"
export REPO_NAME="imazen/imageflow"

# Note: PACKAGE_SUFFIX and NUGET_RUNTIME will be set per runtime in the loop below.
echo "Test environment:"
echo "REL_BINARIES_DIR: ${REL_BINARIES_DIR}"
echo "CI_TAG: ${CI_TAG}"
echo "REPO_NAME: ${REPO_NAME}"
echo "REL_NUGET_OUTPUT_DIR: ${REL_NUGET_OUTPUT_DIR}"

# ---------------------------
# Setup: Change directory to the repository root (or fallback)
# ---------------------------
SAVE_DIR=$(pwd)
# Reasoning: Determine the repo root via git; this is required because pack.sh computes paths relative to it.
cd $(git rev-parse --show-toplevel) || cd $(dirname "$0")/../..
echo "Changed directory to: $(pwd)"

# ---------------------------
# Setup: Create necessary binaries in the expected relative binaries directory.
# ---------------------------
if [ ! -d "$REL_BINARIES_DIR" ]; then
    mkdir -p "$REL_BINARIES_DIR"
fi
echo "Creating mock binaries in ${REL_BINARIES_DIR}"
touch "$REL_BINARIES_DIR/imageflow.dll"
touch "$REL_BINARIES_DIR/imageflow.so"
touch "$REL_BINARIES_DIR/imageflow.dylib"
touch "$REL_BINARIES_DIR/imageflow_tool"
touch "$REL_BINARIES_DIR/imageflow_tool.exe"

if [ ! -d "$REL_NUGET_OUTPUT_DIR" ]; then
    echo "Creating directory ${REL_NUGET_OUTPUT_DIR}"
    mkdir -p "$REL_NUGET_OUTPUT_DIR"
fi

# ---------------------------
# Testing: Loop over multiple target runtimes
# ---------------------------
# Reasoning: Define an array of target runtime values to test pack.sh.
RUNTIMES=("win-arm64" "win-x86" "win-x64" "osx-x64" "osx-arm64" "linux-arm64" "linux-x64")

for runtime in "${RUNTIMES[@]}"; do
    # Reasoning: Set PACKAGE_SUFFIX and NUGET_RUNTIME to the current runtime value.
    export PACKAGE_SUFFIX="$runtime"
    export NUGET_RUNTIME="$runtime"
    
    echo "---------------------------------------------------"
    echo "Running pack.sh for runtime: $runtime"
    echo "PACKAGE_SUFFIX: $PACKAGE_SUFFIX, NUGET_RUNTIME: $NUGET_RUNTIME"
    echo "---------------------------------------------------"
    
    # Reasoning: Invoke the pack.sh script. If it fails for any runtime, exit the test.
    ./ci/pack_nuget/pack.sh || { echo "Failed for runtime $runtime"; exit 1; }
done
