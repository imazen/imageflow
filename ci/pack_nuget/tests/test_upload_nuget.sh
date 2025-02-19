#!/bin/bash
set -euo pipefail

# Create test directory
TEST_DIR=$(mktemp -d)
cleanup() {
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

# Create a fake .nupkg file
echo "Invalid package content" > "$TEST_DIR/test.nupkg"
echo "Invalid package content" > "$TEST_DIR/test2.nupkg"

# Test with invalid API key
echo "Test 1: Testing with invalid API key..."
if ../upload_nuget.sh "$TEST_DIR" "INVALID_API_KEY" > "$TEST_DIR/output.log" 2>&1; then
    echo "❌ Test failed: Expected script to fail with invalid API key"
    exit 1
else
    cat "$TEST_DIR/output.log"
    echo "✅ Test passed: Script failed as expected with invalid API key"
fi

# Test with non-existent directory
echo "Test 2: Testing with non-existent directory..."
if ../upload_nuget.sh "/nonexistent/dir" "INVALID_API_KEY" > "$TEST_DIR/output2.log" 2>&1; then
    echo "❌ Test failed: Expected script to fail with non-existent directory"
    exit 1
else
    cat "$TEST_DIR/output2.log"
    echo "✅ Test passed: Script failed as expected with non-existent directory"
fi

# Test with empty directory
EMPTY_DIR="$TEST_DIR/empty"
mkdir -p "$EMPTY_DIR"
echo "Test 3: Testing with empty directory..."
if ../upload_nuget.sh "$EMPTY_DIR" "INVALID_API_KEY" > "$TEST_DIR/output3.log" 2>&1; then
    echo "❌ Test failed: Expected script to fail with empty directory"
    exit 1
else
    cat "$TEST_DIR/output3.log"
    echo "✅ Test passed: Script failed as expected with empty directory"
fi

echo "All tests completed successfully!" 
