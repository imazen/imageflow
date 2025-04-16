#!/bin/bash
set -e

# Reasoning:
# We need to ensure cleanup is always executed on termination.
# Instead of using EXIT (which in our environment doesn't work reliably),
# we explicitly list the signals to trap.
#
# The selected signals are:
#   • SIGHUP   (hangup, signal number 1)
#   • SIGINT   (interrupt, signal number 2)
#   • SIGQUIT  (quit, signal number 3)
#   • SIGTERM  (termination, signal number 15)
#   • SIGUSR1  (user-defined signal 1)
#   • SIGUSR2  (user-defined signal 2)
#   • SIGABRT  (abort, signal number 6)
#
cleanup() {
    # Capture the exit code as soon as possible.
    exit_code="$?"
    echo "Cleaning up test environment..."
    rm -rf "${TEST_DIR}"
    cd "$SAVE_DIR"
    if [ "$exit_code" -ne 0 ]; then
        echo "❌ Test failed with exit code: $exit_code"
    else
        echo "✅ Test completed successfully"
    fi
    exit "$exit_code"
}

# Trap the specified signals explicitly.
trap cleanup SIGHUP SIGINT SIGQUIT SIGTERM SIGUSR1 SIGUSR2 SIGABRT ERR EXIT

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(dirname "$SCRIPT_DIR")"

# Create unique test directory with timestamp and random number for parallel test safety
TEST_DIR_REL="trash/test_pack_meta_$(date +%s)_$RANDOM"
TEST_DIR="${SCRIPT_DIR}/${TEST_DIR_REL}"
echo "TEST_DIR: ${TEST_DIR}"

# Set up test environment
echo "Creating test environment in: ${TEST_DIR}"
mkdir -p "${TEST_DIR}/artifacts/nuget"

# Set up environment variables
export CI_TAG="v0.9-rc1-1"
export REPO_NAME="imazen/imageflow"
export REL_NUGET_OUTPUT_DIR="ci/pack_nuget/tests/${TEST_DIR_REL}/artifacts/nuget/"
export REL_NUGET_ARCHIVE_DIR="ci/pack_nuget/tests/${TEST_DIR_REL}/archive/"
# Create mock dependency packages
echo "Creating mock dependency packages..."
for rid in win-x64 win-x86 win-arm64 linux-x64 linux-arm64 osx-x64 osx-arm64; do
    mock_package="${TEST_DIR}/artifacts/nuget/Imageflow.NativeRuntime.${rid}.${CI_TAG#v}.nupkg"
    echo "Creating $mock_package"
    touch "$mock_package"
done

echo "Test environment:"
echo "CI_TAG: ${CI_TAG}"
echo "REPO_NAME: ${REPO_NAME}"
echo "REL_NUGET_OUTPUT_DIR: ${REL_NUGET_OUTPUT_DIR}"

SAVE_DIR=$(pwd)
# cd to root of repo, or fallback to current script plus ../..
cd $(git rev-parse --show-toplevel) || cd $(dirname $0)/../..
echo "Changed to $(pwd)"

if [ ! -d "$REL_NUGET_OUTPUT_DIR" ]; then
    echo "Creating dir ${REL_NUGET_OUTPUT_DIR}"
    mkdir -p "$REL_NUGET_OUTPUT_DIR"
fi

# Run pack_meta.sh with modified paths
echo "Running pack_meta.sh..."
./ci/pack_nuget/pack_meta.sh

# Verify all expected packages were created
EXPECTED_PACKAGES=(
    "Imageflow.NativeRuntime.All"
    "Imageflow.NativeRuntime.All.x64"
    "Imageflow.NativeRuntime.All.Arm64"
    "Imageflow.NativeRuntime.All.Windows"
    "Imageflow.NativeRuntime.All.Linux"
    "Imageflow.NativeRuntime.All.Mac"
    "Imageflow.Net.All"
    "Imageflow.Net.All.x64"
    "Imageflow.Net.All.Arm64"
    "Imageflow.Net.All.Windows"
    "Imageflow.Net.All.Linux"
    "Imageflow.Net.All.Mac"
)

echo -e "\nVerifying created packages:"
for package in "${EXPECTED_PACKAGES[@]}"; do
    package_file="${TEST_DIR}/artifacts/nuget/${package}.${CI_TAG#v}.nupkg"
    if [[ -f "$package_file" ]] && [[ -s "$package_file" ]]; then
        echo "✓ $package created successfully"
    else
        echo "❌ Failed to create $package"
        echo "Expected file: $package_file"
        echo "Directory contents:"
        ls -la "${TEST_DIR}/artifacts/nuget/"
        exit 1
    fi
done


# Attempt upload if NUGET_API_KEY and REALLY_UPLOAD_BAD_NUGET_FILES is set
if [ -n "$NUGET_API_KEY" ] && [ "${REALLY_UPLOAD_BAD_NUGET_FILES:-}" = "true" ]; then
    echo "Attempting to upload then delete packages to nuget.org..."
    DELETE_FROM_NUGET_AFTER_UPLOAD=true ./ci/pack_nuget/upload_nuget.sh "${TEST_DIR}/artifacts/nuget/" "$NUGET_API_KEY" 2>&1
else
    echo "NUGET_API_KEY and REALLY_UPLOAD_BAD_NUGET_FILES=true are not set, skipping real upload"
fi

echo "All packages created successfully" 
