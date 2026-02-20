#!/bin/bash
set -e
set -o pipefail

# Get the directory of the current script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ZIPIT_SCRIPT="${SCRIPT_DIR}/pack_nuget/zipit.sh"

# Check if zipit.sh exists and is executable
if [ ! -x "$ZIPIT_SCRIPT" ]; then
    echo "Error: Archiving script '$ZIPIT_SCRIPT' not found or not executable." >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Required environment variables (should be passed from the workflow):
# - TARGET_DIR: Directory containing build artifacts
# - REL_BINARIES_DIR: Directory containing release binaries
# - EXTENSION: Either 'zip' or 'tar.gz'
# - IMAGEFLOW_TAG_SHA_SUFFIX: Used for naming artifacts
# - LIBIMAGEFLOW_STATIC: Static library name
# - LIBIMAGEFLOW_DYNAMIC: Dynamic library name
# - IMAGEFLOW_TOOL: Tool name (e.g. imageflow_tool or imageflow_tool.exe)
# - TAG_SHA_SUFFIX: Suffix for the current matrix build
# - GITHUB_SHA: Git commit SHA
# - GITHUB_REF_NAME: Git ref name (tag/branch)
# - MATRIX_COMMIT_SUFFIX: Suffix for the current matrix build
# ------------------------------------------------------------------------------

# Validate required environment variables
required_vars=(
    "TARGET_DIR"
    "REL_BINARIES_DIR"
    "EXTENSION"
    "IMAGEFLOW_TAG_SHA_SUFFIX"
    "LIBIMAGEFLOW_STATIC"
    "LIBIMAGEFLOW_DYNAMIC"
    "IMAGEFLOW_TOOL"
    "TAG_SHA_SUFFIX"
    "GITHUB_SHA"
    "GITHUB_REF_NAME"
    "MATRIX_COMMIT_SUFFIX"
    "HTTPS_UPLOAD_BASE"
)


for var in "${required_vars[@]}"; do
    if [ -z "${!var}" ]; then
        echo "Error: Required environment variable $var is not set"
        exit 1
    fi
done

# require REL_BINARIES_DIR to end in a slash and exist
if [[ "${REL_BINARIES_DIR}" != */ ]]; then
    echo "Error: REL_BINARIES_DIR must end in a slash"
    exit 1
fi
if [ ! -d "${REL_BINARIES_DIR}" ]; then
    echo "Error: REL_BINARIES_DIR does not exist"
    exit 1
fi

# Create required directories
mkdir -p ./artifacts/github
mkdir -p ./artifacts/temp
mkdir -p ./artifacts/upload/releases/${GITHUB_REF_NAME}
mkdir -p ./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}
mkdir -p ./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}
mkdir -p ./artifacts/static-staging
mkdir -p ./artifacts/staging/headers  # Explicitly create headers directory

# ------------------------------------------------------------------------------
# Package documentation (if exists and is non-empty)
# ------------------------------------------------------------------------------
if [ -d "./${TARGET_DIR}doc" ] && [ -n "$(ls -A "./${TARGET_DIR}doc" 2>/dev/null)" ]; then
    (
        cd "./${TARGET_DIR}doc"
        mkdir -p "$(pwd)/../../artifacts/staging"
        tar czf "$(pwd)/../../artifacts/staging/docs.${EXTENSION}" ./*
    )
    echo "Documentation packaged successfully"
else
    echo "Documentation directory not found or empty - skipping documentation packaging"
fi

# ------------------------------------------------------------------------------
# Copy binaries (and symbols) and headers (with strict checking)
# ------------------------------------------------------------------------------
# List all files to be copied
echo "Copying files from ${REL_BINARIES_DIR} and bindings/headers"
ls "./${REL_BINARIES_DIR}"libimageflow* | cat
ls "./${REL_BINARIES_DIR}"imageflow_* | cat
ls bindings/headers/*.h | cat
ls bindings/headers/imageflow_default.h | cat
echo "--------------------------------"
cp bindings/headers/*.h ./artifacts/staging/headers/
cp bindings/headers/imageflow_default.h ./artifacts/staging/imageflow.h

# either static or dynamic library should exist
cp -R "./${REL_BINARIES_DIR}"${LIBIMAGEFLOW_DYNAMIC} ./artifacts/staging/ || cp -R "./${REL_BINARIES_DIR}"${LIBIMAGEFLOW_STATIC} ./artifacts/staging/
# tool should always exist
cp -R "./${REL_BINARIES_DIR}"${IMAGEFLOW_TOOL} ./artifacts/staging/


# Copy imageflow.dll.lib if it exists
if [[ "${LIBIMAGEFLOW_DYNAMIC}" == "imageflow.dll" ]]; then
    cp -R "./${REL_BINARIES_DIR}"imageflow.dll.lib ./artifacts/staging/ || true
fi

# Function to handle debug symbols for a specific binary
handle_debug_symbols() {
    local binary="$1"
    local platform="$2"
    local symbol_type="$3"
    local symbol_ext="$4"
    
    local source_dir="./${REL_BINARIES_DIR}"
    local dest_dir="./artifacts/staging"
    
    case "$platform" in
        "macos")
            local symbol_path="${source_dir}${binary}.${symbol_ext}"
            if [ -d "$symbol_path" ]; then
                echo "Found ${symbol_type} directory: $symbol_path"
                cp -R "$symbol_path" "${dest_dir}/"
            fi
            ;;
        "linux")
            local symbol_path="${source_dir}${binary}.${symbol_ext}"
            if [ -f "$symbol_path" ]; then
                echo "Found ${symbol_type} file: $symbol_path"
                cp "$symbol_path" "${dest_dir}/"
            fi
            ;;
        "windows")
            local symbol_path="${source_dir}${binary%.*}.${symbol_ext}"
            if [ -f "$symbol_path" ]; then
                echo "Found ${symbol_type} file: $symbol_path"
                cp "$symbol_path" "${dest_dir}/"
            fi
            ;;
    esac
}

# Function to verify debug symbols for a specific binary
verify_debug_symbols() {
    local binary="$1"
    local platform="$2"
    local symbol_type="$3"
    local symbol_ext="$4"
    
    local dest_dir="./artifacts/staging"
    
    case "$platform" in
        "macos")
            local symbol_path="${dest_dir}/${binary}.${symbol_ext}"
            if [ -d "$symbol_path" ]; then
                echo "✓ Verified ${symbol_type} directory in staging: $symbol_path"
                ls -R "$symbol_path"
            else
                echo "✗ Missing ${symbol_type} directory for $binary: $symbol_path not found"
            fi
            ;;
        "windows")
            local symbol_path="${dest_dir}/${binary%.*}.${symbol_ext}"
            if [ -f "$symbol_path" ]; then
                echo "✓ Verified ${symbol_type} file in staging: $symbol_path"
                ls -l "$symbol_path"
            else
                echo "✗ Missing ${symbol_type} file for $binary: $symbol_path not found"
            fi
            ;;
        "linux")
            local symbol_path="${dest_dir}/${binary}.${symbol_ext}"
            if [ -f "$symbol_path" ]; then
                echo "✓ Verified ${symbol_type} file in staging: $symbol_path"
                ls -l "$symbol_path"
            else
                echo "✗ Missing ${symbol_type} file for $binary: $symbol_path not found"
            fi
            ;;
    esac
}

# Handle debug symbols based on platform
if [[ "${LIBIMAGEFLOW_DYNAMIC}" == "libimageflow.dylib" ]]; then
    echo "Copying debug symbols for macOS build..."
    handle_debug_symbols "libimageflow.dylib" "macos" ".dSYM" "dSYM"
    handle_debug_symbols "imageflow_tool" "macos" ".dSYM" "dSYM"
    
    echo "Verifying debug symbols in staging..."
    verify_debug_symbols "libimageflow.dylib" "macos" ".dSYM" "dSYM"
    verify_debug_symbols "imageflow_tool" "macos" ".dSYM" "dSYM"
elif [[ "${LIBIMAGEFLOW_DYNAMIC}" == "libimageflow.so" ]]; then
    echo "Copying debug symbols for Linux build..."
    handle_debug_symbols "libimageflow.so" "linux" ".dwp" "dwp"
    handle_debug_symbols "imageflow_tool" "linux" ".dwp" "dwp"
    
    echo "Verifying debug symbols in staging..."
    verify_debug_symbols "libimageflow.so" "linux" ".dwp" "dwp"
    verify_debug_symbols "imageflow_tool" "linux" ".dwp" "dwp"
elif [[ "${LIBIMAGEFLOW_DYNAMIC}" == "imageflow.dll" ]]; then
    echo "Copying debug symbols for Windows build..."
    handle_debug_symbols "imageflow.dll" "windows" ".pdb" "pdb"
    handle_debug_symbols "imageflow_tool.exe" "windows" ".pdb" "pdb"
    
    echo "Verifying debug symbols in staging..."
    verify_debug_symbols "imageflow.dll" "windows" ".pdb" "pdb"
    verify_debug_symbols "imageflow_tool.exe" "windows" ".pdb" "pdb"
fi

# Verify and copy installation scripts, IF LIBIMAGEFLOW_DYNAMIC!=imageflow.dll
if [ "${LIBIMAGEFLOW_DYNAMIC}" != "imageflow.dll" ]; then
    for script in "./ci/packaging_extras/"{install,uninstall}.sh; do
        if [ ! -f "$script" ]; then
            echo "Error: Required installation script not found: $script"
            exit 1
        fi
        cp "$script" ./artifacts/staging/
    done
fi

# Clean up unnecessary files
rm ./artifacts/staging/*.{o,d,rlib} 2>/dev/null || true
mv "./artifacts/staging/${LIBIMAGEFLOW_STATIC}" "./artifacts/static-staging/${LIBIMAGEFLOW_STATIC}" 2>/dev/null || true
rm ./artifacts/staging/*-* 2>/dev/null || true

# ------------------------------------------------------------------------------
# Create main archive
# ------------------------------------------------------------------------------
TEMP_ARCHIVE_NAME="./artifacts/temp/archive.${EXTENSION}"
# Use '.' to include all content relative to the staging directory
"$ZIPIT_SCRIPT" "$TEMP_ARCHIVE_NAME" "./artifacts/staging" "."

# ------------------------------------------------------------------------------
# Making release archive copies
# ------------------------------------------------------------------------------
# Report pwd
echo "Copying release archives. Current directory: $(pwd)"
mkdir -p "$(dirname "./artifacts/github/${IMAGEFLOW_TAG_SHA_SUFFIX}.${EXTENSION}")"
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/github/${IMAGEFLOW_TAG_SHA_SUFFIX}.${EXTENSION}"
mkdir -p "$(dirname "./artifacts/upload/releases/${GITHUB_REF_NAME}/${IMAGEFLOW_TAG_SHA_SUFFIX}.${EXTENSION}")"
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/upload/releases/${GITHUB_REF_NAME}/${IMAGEFLOW_TAG_SHA_SUFFIX}.${EXTENSION}"
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}/imageflow.${EXTENSION}"
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}/imageflow.${EXTENSION}"

# ------------------------------------------------------------------------------
# Handle static library if it exists
# ------------------------------------------------------------------------------
TEMP_STATIC_LIB_PATH="./artifacts/static-staging/${LIBIMAGEFLOW_STATIC}"
if [ -f "${TEMP_STATIC_LIB_PATH}" ]; then
    TEMP_STATIC_ARCHIVE="./artifacts/temp/staticlib-${LIBIMAGEFLOW_STATIC}.${EXTENSION}"
    FILE_NAME="staticlib-${LIBIMAGEFLOW_STATIC}.${EXTENSION}" # Renamed variable for clarity
    # Archive just the static library file, relative to its directory
    "$ZIPIT_SCRIPT" "$TEMP_STATIC_ARCHIVE" "./artifacts/static-staging" "${LIBIMAGEFLOW_STATIC}"

    # Create static archive directories and copy files
    # Note: File name for copy includes 'staticlib-' prefix from FILE_NAME variable
    mkdir -p "$(dirname "./artifacts/upload/releases/${GITHUB_REF_NAME}/staticlib-${TAG_SHA_SUFFIX}.${FILE_NAME}")"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/releases/${GITHUB_REF_NAME}/staticlib-${TAG_SHA_SUFFIX}.${FILE_NAME}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    mkdir -p "$(dirname "./artifacts/github/staticlib-${TAG_SHA_SUFFIX}.${FILE_NAME}")"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/github/staticlib-${TAG_SHA_SUFFIX}.${FILE_NAME}"
fi

# ------------------------------------------------------------------------------
# Make a single-file imageflow_tool archive that extracts without a directory
# ------------------------------------------------------------------------------
TEMP_TOOL_PATH="./artifacts/staging/${IMAGEFLOW_TOOL}" # Corrected path, tool is in staging
if [ -f "${TEMP_TOOL_PATH}" ]; then
    TEMP_TOOL_ARCHIVE="./artifacts/temp/tool-${IMAGEFLOW_TOOL}.${EXTENSION}" # Store archive elsewhere
    FILE_NAME="${IMAGEFLOW_TOOL}.${EXTENSION}" #imageflow_tool.tar.gz or imageflow_tool.exe.zip
    # Archive just the tool, relative to the staging directory
    "$ZIPIT_SCRIPT" "$TEMP_TOOL_ARCHIVE" "./artifacts/staging" "${IMAGEFLOW_TOOL}"

    # Create static archive directories and copy files
    # Note: File name for copy includes 'tool-' prefix
    mkdir -p "$(dirname "./artifacts/upload/releases/${GITHUB_REF_NAME}/tool-${TAG_SHA_SUFFIX}.${FILE_NAME}")"
    cp "${TEMP_TOOL_ARCHIVE}" "./artifacts/upload/releases/${GITHUB_REF_NAME}/tool-${TAG_SHA_SUFFIX}.${FILE_NAME}"
    cp "${TEMP_TOOL_ARCHIVE}" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    cp "${TEMP_TOOL_ARCHIVE}" "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    mkdir -p "$(dirname "./artifacts/github/tool-${TAG_SHA_SUFFIX}.${FILE_NAME}")"
    cp "${TEMP_TOOL_ARCHIVE}" "./artifacts/github/tool-${TAG_SHA_SUFFIX}.${FILE_NAME}"
fi

# List created artifacts
echo "GitHub release artifacts:"
ls -R ./artifacts/github | cat
echo "--------------------------------"
echo "Upload artifacts:"
ls -R ./artifacts/upload | cat
echo "--------------------------------"

# List expected final URLS (based on contents of artifacts/uplod, recursive)
# HTTPS_UPLOAD_BASE doesn't have a trailing slash
echo "Expected final URLs:"
find ./artifacts/upload -type f -print0 | while IFS= read -r -d '' file; do
    echo "${HTTPS_UPLOAD_BASE}/${file#./artifacts/upload/}"
done
