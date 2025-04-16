#!/bin/bash
set -e
set -o pipefail

# Function to create an archive (.zip or .tar.gz) from a specific directory
# Usage: create_archive <output_archive_path> <base_directory> <path_to_include>
#   output_archive_path: Absolute or relative path for the final archive file.
#   base_directory: The directory to change into before archiving. Paths inside the archive will be relative to this.
#   path_to_include: The specific file or directory (or '.') within base_directory to add to the archive.
create_archive() {
    local output_archive_path="$1"
    local base_directory="$2"
    local path_to_include="$3"
    local original_dir=$(pwd)
    local absolute_output_path

    # Resolve absolute path for the output archive
    if [[ "$output_archive_path" == /* ]]; then
        absolute_output_path="$output_archive_path"
    else
        absolute_output_path="$original_dir/$output_archive_path"
    fi

    # Ensure base directory exists
    if [ ! -d "$base_directory" ]; then
        echo "Error: Base directory '$base_directory' does not exist." >&2
        return 1
    fi

    echo "Creating archive '$absolute_output_path' from base '$base_directory' including '$path_to_include'"

    # Change into the base directory
    cd "$base_directory" || { echo "Error: Failed to cd into '$base_directory'." >&2; cd "$original_dir"; return 1; }

    # Create archive based on extension
    if [[ "$absolute_output_path" == *.zip ]]; then
        echo "Using zip..."
        if ! zip -r -q "$absolute_output_path" "$path_to_include"; then
            echo "Error: zip command failed for '$absolute_output_path'" >&2
            cd "$original_dir"
            return 1
        fi
    elif [[ "$absolute_output_path" == *.tar.gz ]]; then
        echo "Using tar..."
        # Use --transform to remove leading ./ if path_to_include is '.'
        local tar_opts=""
        if [[ "$path_to_include" == "." ]]; then
            tar_opts="--transform=s/^\.\///"
        fi
        if ! tar $tar_opts -czf "$absolute_output_path" "$path_to_include"; then
            echo "Error: tar command failed for '$absolute_output_path'" >&2
            cd "$original_dir"
            return 1
        fi
    else
        echo "Error: Unsupported archive extension for '$absolute_output_path'. Use .zip or .tar.gz." >&2
        cd "$original_dir"
        return 1
    fi

    # Return to the original directory
    cd "$original_dir" || { echo "Error: Failed to cd back to original directory '$original_dir'." >&2; return 1; }

    echo "Successfully created archive '$absolute_output_path'"
    return 0
}

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
cp -R "./${REL_BINARIES_DIR}"libimageflow* ./artifacts/staging/ || true
cp -R "./${REL_BINARIES_DIR}"imageflow_* ./artifacts/staging/
# tool should always exist
cp -R "./${REL_BINARIES_DIR}"${IMAGEFLOW_TOOL} ./artifacts/staging/
cp bindings/headers/*.h ./artifacts/staging/headers/
cp bindings/headers/imageflow_default.h ./artifacts/staging/imageflow.h

# Verify and copy installation scripts
for script in "./ci/packaging_extras/"{install,uninstall}.sh; do
    if [ ! -f "$script" ]; then
        echo "Error: Required installation script not found: $script"
        exit 1
    fi
    cp "$script" ./artifacts/staging/
done

# Clean up unnecessary files
rm ./artifacts/staging/*.{o,d,rlib} 2>/dev/null || true
mv "./artifacts/staging/${LIBIMAGEFLOW_STATIC}" "./artifacts/static-staging/${LIBIMAGEFLOW_STATIC}" 2>/dev/null || true
rm ./artifacts/staging/*-* 2>/dev/null || true

# ------------------------------------------------------------------------------
# Create main archive
# ------------------------------------------------------------------------------
TEMP_ARCHIVE_NAME="./artifacts/temp/archive.${EXTENSION}"
# Use '.' to include all content relative to the staging directory
create_archive "$TEMP_ARCHIVE_NAME" "./artifacts/staging" "."

# ------------------------------------------------------------------------------
# Create release archives
# ------------------------------------------------------------------------------
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/github/${IMAGEFLOW_TAG_SHA_SUFFIX}.${EXTENSION}"
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
    create_archive "$TEMP_STATIC_ARCHIVE" "./artifacts/static-staging" "${LIBIMAGEFLOW_STATIC}"

    # Create static archive directories and copy files
    # Note: File name for copy includes 'staticlib-' prefix from FILE_NAME variable
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/releases/${GITHUB_REF_NAME}/staticlib-${TAG_SHA_SUFFIX}.${FILE_NAME}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
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
    create_archive "$TEMP_TOOL_ARCHIVE" "./artifacts/staging" "${IMAGEFLOW_TOOL}"

    # Create static archive directories and copy files
    # Note: File name for copy includes 'tool-' prefix
    cp "${TEMP_TOOL_ARCHIVE}" "./artifacts/upload/releases/${GITHUB_REF_NAME}/tool-${TAG_SHA_SUFFIX}.${FILE_NAME}"
    cp "${TEMP_TOOL_ARCHIVE}" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    cp "${TEMP_TOOL_ARCHIVE}" "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
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
