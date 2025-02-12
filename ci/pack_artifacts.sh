#!/bin/bash
set -e

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
TEMP_ARCHIVE_NAME="./artifacts/staging/archive.${EXTENSION}"
(
    cd ./artifacts/staging
    tar czf "./archive.${EXTENSION}" ./*
)

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
TEMP_STATIC_LIB="./artifacts/static-staging/${LIBIMAGEFLOW_STATIC}"
if [ -f "${TEMP_STATIC_LIB}" ]; then
    TEMP_STATIC_ARCHIVE="./artifacts/static-staging/${LIBIMAGEFLOW_STATIC}.${EXTENSION}"
    FILE_NAME="staticlib-${LIBIMAGEFLOW_STATIC}.${EXTENSION}"
    tar czf "${TEMP_STATIC_ARCHIVE}" "${TEMP_STATIC_LIB}"
    # Create static archive directories and copy files
    # Create static archive directories and copy files
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/releases/${GITHUB_REF_NAME}/staticlib-${TAG_SHA_SUFFIX}.${FILE_NAME}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/commits/latest/${MATRIX_COMMIT_SUFFIX}/${FILE_NAME}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/github/staticlib-${TAG_SHA_SUFFIX}.${FILE_NAME}"
fi

# ------------------------------------------------------------------------------
# Make a single-file imageflow_tool archive that extracts without a directory
# ------------------------------------------------------------------------------
TEMP_TOOL="./artifacts/staging/${IMAGEFLOW_TOOL}"
if [ -f "${TEMP_TOOL}" ]; then
    TEMP_TOOL_ARCHIVE="./artifacts/static-staging/imageflow_tool.${EXTENSION}"
    FILE_NAME="${IMAGEFLOW_TOOL}.${EXTENSION}" #imageflow_tool.tar.gz or imageflow_tool.exe.zip
    tar czf "${TEMP_TOOL_ARCHIVE}" "${TEMP_TOOL}"

    # Create static archive directories and copy files

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
