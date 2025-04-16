#!/bin/bash

set -e
set -o pipefail

# Get directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
TRASH_DIR="${SCRIPT_DIR}/trash/zipit_test_$(date +%s)_${RANDOM}"


# --- Helper Functions ---

# Function to check if a path exists inside the extraction directory
# Usage: assert_exists <extraction_dir> <path_relative_to_extraction_dir>
assert_exists() {
    local extract_dir="$1"
    local check_path="$2"
    if [ ! -e "${extract_dir}/${check_path}" ]; then
        echo "❌ FAIL: Expected path '${check_path}' not found in archive ($extract_dir)" >&2
        ls -R "$extract_dir"
        return 1
    fi
    echo "✓ Found: ${check_path}"
    return 0
}

# Function to check if a path DOES NOT exist inside the extraction directory
# Usage: assert_not_exists <extraction_dir> <path_relative_to_extraction_dir>
assert_not_exists() {
    local extract_dir="$1"
    local check_path="$2"
    if [ -e "${extract_dir}/${check_path}" ]; then
        echo "❌ FAIL: Unexpected path '${check_path}' found in archive ($extract_dir)" >&2
        ls -R "$extract_dir"
        return 1
    fi
     echo "✓ Absent: ${check_path}"
    return 0
}

# --- Setup ---

echo "Setting up test environment in ${TRASH_DIR}..."
rm -rf "${TRASH_DIR}"
mkdir -p "${TRASH_DIR}/base/headers"
mkdir -p "${TRASH_DIR}/extract/zip_all"
mkdir -p "${TRASH_DIR}/extract/tar_all"
mkdir -p "${TRASH_DIR}/extract/zip_headers"
mkdir -p "${TRASH_DIR}/extract/tar_headers"

# Create test files
echo "Header content" > "${TRASH_DIR}/base/headers/file.h"
echo "Other content" > "${TRASH_DIR}/base/other.txt"
echo ".dotfile content" > "${TRASH_DIR}/base/.dotfile"

echo "Test files created:"
ls -R "${TRASH_DIR}/base"

# Define archive paths
ARCHIVE_ZIP_ALL="${TRASH_DIR}/all_content.zip"
ARCHIVE_TAR_ALL="${TRASH_DIR}/all_content.tar.gz"
ARCHIVE_ZIP_HEADERS="${TRASH_DIR}/headers_only.zip"
ARCHIVE_TAR_HEADERS="${TRASH_DIR}/headers_only.tar.gz"

# --- Cleanup Trap ---
cleanup() {
    local exit_code=$?
    echo "Cleaning up test environment ${TRASH_DIR}..."
    rm -rf "${TRASH_DIR}"
    if [ $exit_code -ne 0 ]; then
        echo "❌ Test script failed with exit code: $exit_code"
    else
        echo "✅ Test script completed successfully"
    fi
    exit $exit_code
}
trap cleanup EXIT ERR INT TERM

# --- Test Execution ---
FAILED_ASSERTIONS=0

# Test 1: Create zip of all content
echo -e "\n--- Test 1: Create zip of all content ('.') ---"
if ! "${PACK_DIR}/zipit.sh" "$ARCHIVE_ZIP_ALL" "${TRASH_DIR}/base" "."; then
    echo "❌ FAIL: zipit.sh failed for zip all content" >&2
    FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
else
    echo "Extracting ${ARCHIVE_ZIP_ALL}..."
    unzip -q "$ARCHIVE_ZIP_ALL" -d "${TRASH_DIR}/extract/zip_all"
    assert_exists "${TRASH_DIR}/extract/zip_all" "headers/file.h" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_exists "${TRASH_DIR}/extract/zip_all" "other.txt" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_exists "${TRASH_DIR}/extract/zip_all" ".dotfile" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/zip_all" "base" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
fi

# Test 2: Create tar.gz of all content
echo -e "\n--- Test 2: Create tar.gz of all content ('.') ---"
if ! "${PACK_DIR}/zipit.sh" "$ARCHIVE_TAR_ALL" "${TRASH_DIR}/base" "."; then
    echo "❌ FAIL: zipit.sh failed for tar all content" >&2
    FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
else
    echo "Extracting ${ARCHIVE_TAR_ALL}..."
    tar -xzf "$ARCHIVE_TAR_ALL" -C "${TRASH_DIR}/extract/tar_all"
    assert_exists "${TRASH_DIR}/extract/tar_all" "headers/file.h" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_exists "${TRASH_DIR}/extract/tar_all" "other.txt" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_exists "${TRASH_DIR}/extract/tar_all" ".dotfile" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/tar_all" "base" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
fi

# Test 3: Create zip of headers only
echo -e "\n--- Test 3: Create zip of headers directory ---"
if ! "${PACK_DIR}/zipit.sh" "$ARCHIVE_ZIP_HEADERS" "${TRASH_DIR}/base" "headers"; then
    echo "❌ FAIL: zipit.sh failed for zip headers only" >&2
    FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
else
    echo "Extracting ${ARCHIVE_ZIP_HEADERS}..."
    unzip -q "$ARCHIVE_ZIP_HEADERS" -d "${TRASH_DIR}/extract/zip_headers"
    assert_exists "${TRASH_DIR}/extract/zip_headers" "headers/file.h" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/zip_headers" "other.txt" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/zip_headers" ".dotfile" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/zip_headers" "base" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
fi

# Test 4: Create tar.gz of headers only
echo -e "\n--- Test 4: Create tar.gz of headers directory ---"
if ! "${PACK_DIR}/zipit.sh" "$ARCHIVE_TAR_HEADERS" "${TRASH_DIR}/base" "headers"; then
    echo "❌ FAIL: zipit.sh failed for tar headers only" >&2
    FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
else
    echo "Extracting ${ARCHIVE_TAR_HEADERS}..."
    tar -xzf "$ARCHIVE_TAR_HEADERS" -C "${TRASH_DIR}/extract/tar_headers"
    assert_exists "${TRASH_DIR}/extract/tar_headers" "headers/file.h" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/tar_headers" "other.txt" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/tar_headers" ".dotfile" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
    assert_not_exists "${TRASH_DIR}/extract/tar_headers" "base" || FAILED_ASSERTIONS=$((FAILED_ASSERTIONS + 1))
fi

# --- Final Verdict ---
if [ $FAILED_ASSERTIONS -eq 0 ]; then
    echo -e "\n✅ All zipit tests passed."
    exit 0
else
    echo -e "\n❌ ${FAILED_ASSERTIONS} zipit test assertions failed."
    exit 1
fi 
