#!/bin/bash

# Load utils
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SAVE_DIR=$(pwd)
cd "$SCRIPT_DIR"

source ../utils.sh

# Create unique test directory and output file names
TEST_DIR="trash/test_package_$(date +%s)_$RANDOM"
OUTPUT_FILE="trash/test_output_$(date +%s)_$RANDOM.nupkg"

# Ensure cleanup happens even on failure
cleanup() {
    local exit_code=$?
    echo "Cleaning up test environment..."
    rm -rf "$TEST_DIR"
    rm -f "$OUTPUT_FILE"
    if [ $exit_code -ne 0 ]; then
        echo "❌ Test failed with exit code: $exit_code"
    else
        echo "✅ Test completed successfully"
    fi
    cd "$SAVE_DIR"
    exit $exit_code
}
trap cleanup  1 2 3 6

echo "Checking available compression tools..."
if command -v zip >/dev/null 2>&1; then
    echo "✓ zip is available"
else
    echo "✗ zip is not available"
fi

if command -v 7z >/dev/null 2>&1; then
    echo "✓ 7z is available"
else
    echo "✗ 7z is not available"
fi

if command -v powershell.exe >/dev/null 2>&1; then
    echo "✓ powershell.exe is available"
else
    echo "✗ powershell.exe is not available"
fi

# Create test directory structure
echo -e "\nCreating test directory structure..."
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR/content"

# Create some test files
echo "Creating test files..."
echo "Test content 1" > "$TEST_DIR/content/file1.txt"
echo "Test content 2" > "$TEST_DIR/content/file2.txt"

# List created files
echo "Created files:"
find "$TEST_DIR" -type f -exec ls -l {} \;

# Create test package
echo -e "\nTesting package creation..."
rm -f "$OUTPUT_FILE"

# Capture all output
if ! create_package "$OUTPUT_FILE" "$TEST_DIR" 2>&1; then
    echo "Package creation failed with error code: $?"
    exit 1
fi

# Verify package was created
if [[ -f "$OUTPUT_FILE" ]] && [[ -s "$OUTPUT_FILE" ]]; then
    echo "✅ Package created successfully"
    ls -l "$OUTPUT_FILE"
else
    echo "❌ Package creation failed - file not found or empty"
    ls -la .
    exit 1
fi 
