#!/bin/bash
set -e #Exit on failure.

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(dirname "$SCRIPT_DIR")"

TEST_DIR_REL="trash/test_pack_$(date +%s)_$RANDOM"
# Create unique test directory
TEST_DIR="${SCRIPT_DIR}/${TEST_DIR_REL}"
echo "TEST_DIR: ${TEST_DIR}"

# Ensure cleanup happens even on failure
cleanup() {
    local exit_code=$?
    echo "Cleaning up test environment..."
    rm -rf "${TEST_DIR}"
    cd $SAVE_DIR
    if [ $exit_code -ne 0 ]; then
        echo "❌ Test failed with exit code: $exit_code"
    else
        echo "✅ Test completed successfully"
    fi
    exit $exit_code
}
trap cleanup  1 2 3 6

# Set up test environment
echo "Creating test environment in: ${TEST_DIR}"
mkdir -p "${TEST_DIR}/binaries"

# Create mock binary files
touch "${TEST_DIR}/binaries/imageflow.dll"
touch "${TEST_DIR}/binaries/imageflow_tool.exe"

# Set up environment - ensure BINARIES_DIR ends with a slash
export REL_BINARIES_DIR="ci/pack_nuget/tests/${TEST_DIR_REL}/binaries/"
export REL_NUGET_OUTPUT_DIR="ci/pack_nuget/tests/${TEST_DIR_REL}/nuget/"
export CI_TAG="v0.9-rc1-1"
export PACKAGE_SUFFIX="win-x64"
export NUGET_RUNTIME="win-x64"
export REPO_NAME="imazen/imageflow"

echo "Test environment:"
echo "REL_BINARIES_DIR: ${REL_BINARIES_DIR}"
echo "CI_TAG: ${CI_TAG}"
echo "PACKAGE_SUFFIX: ${PACKAGE_SUFFIX}"
echo "NUGET_RUNTIME: ${NUGET_RUNTIME}"
echo "REPO_NAME: ${REPO_NAME}"
echo "REL_NUGET_OUTPUT_DIR: ${REL_NUGET_OUTPUT_DIR}"

SAVE_DIR=$(pwd)
# cd to root of repo, or fallback to current script plus ../..
cd $(git rev-parse --show-toplevel) || cd $(dirname $0)/../..
echo "Changed to $(pwd)"
# if BINARIES_DIR doesn't exist, relative to root of repo, run cargo build --release 
if [ ! -d "$REL_BINARIES_DIR" ]; then
    # create imageflow.dll, .so, .dylin, imageflow_tool, and imageflow_tool.exe in BINARIES_DIR with touch
    mkdir -p "$REL_BINARIES_DIR"
fi
echo "Creating mock binaries in ${REL_BINARIES_DIR}"
touch "$REL_BINARIES_DIR/imageflow.dll"
touch "$REL_BINARIES_DIR/imageflow.so"
touch "$REL_BINARIES_DIR/imageflow.dylib"
touch "$REL_BINARIES_DIR/imageflow_tool"
touch "$REL_BINARIES_DIR/imageflow_tool.exe"
if [ ! -d "$REL_NUGET_OUTPUT_DIR" ]; then
    echo "Creating dir ${REL_NUGET_OUTPUT_DIR}"
    mkdir -p "$REL_NUGET_OUTPUT_DIR"
fi
./ci/pack_nuget/pack.sh
