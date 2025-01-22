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
    "GITHUB_SHA"
    "GITHUB_REF_NAME"
    "MATRIX_COMMIT_SUFFIX"
)

for var in "${required_vars[@]}"; do
    if [ -z "${!var}" ]; then
        echo "Error: Required environment variable $var is not set"
        exit 1
    fi
done

# Create required directories
mkdir -p ./artifacts/{staging/{headers},github,upload/{releases/${GITHUB_REF_NAME},commits/${GITHUB_SHA}}}
mkdir -p "./artifacts/static-staging"

# ------------------------------------------------------------------------------
# Package documentation
# ------------------------------------------------------------------------------
if [ -d "./${TARGET_DIR}doc" ]; then
    (
        cd "./${TARGET_DIR}doc"
        tar czf "../docs.${EXTENSION}" ./*
    )
    mv "./${TARGET_DIR}docs.${EXTENSION}" ./artifacts/staging/
fi

# ------------------------------------------------------------------------------
# Copy binaries and headers
# ------------------------------------------------------------------------------
cp -R "./${REL_BINARIES_DIR}"/{imageflow_,libimageflow}* ./artifacts/staging/
cp bindings/headers/*.h ./artifacts/staging/headers/
cp bindings/headers/imageflow_default.h ./artifacts/staging/imageflow.h
cp ./ci/packaging_extras/{install,uninstall}.sh ./artifacts/staging/

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
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/github/imageflow-${IMAGEFLOW_TAG_SHA_SUFFIX}.${EXTENSION}"
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/upload/releases/${GITHUB_REF_NAME}/${IMAGEFLOW_TAG_SHA_SUFFIX}.${EXTENSION}"
cp "${TEMP_ARCHIVE_NAME}" "./artifacts/upload/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}.${EXTENSION}"

# ------------------------------------------------------------------------------
# Handle static library if it exists
# ------------------------------------------------------------------------------
TEMP_STATIC_LIB="./artifacts/static-staging/${LIBIMAGEFLOW_STATIC}"
if [ -f "${TEMP_STATIC_LIB}" ]; then
    TEMP_STATIC_ARCHIVE="./artifacts/static-staging/${LIBIMAGEFLOW_STATIC}.${EXTENSION}"
    tar czf "${TEMP_STATIC_ARCHIVE}" -C "$(dirname "${TEMP_STATIC_LIB}")" "$(basename "${TEMP_STATIC_LIB}")"

    # Create static archive directories and copy files
    mkdir -p ./artifacts/upload/static/${MATRIX_TARGET}/{latest,releases/${GITHUB_REF_NAME},commits/${GITHUB_SHA}}
    
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/static/${MATRIX_TARGET}/latest/${LIBIMAGEFLOW_STATIC}.${EXTENSION}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/static/${MATRIX_TARGET}/releases/${GITHUB_REF_NAME}/${LIBIMAGEFLOW_STATIC}.${EXTENSION}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/upload/static/${MATRIX_TARGET}/commits/${GITHUB_SHA}/${MATRIX_COMMIT_SUFFIX}.${EXTENSION}"
    cp "${TEMP_STATIC_ARCHIVE}" "./artifacts/github/lib${IMAGEFLOW_TAG_SHA_SUFFIX}-${MATRIX_COMMIT_SUFFIX}.${LIBIMAGEFLOW_STATIC}.${EXTENSION}"
fi

# List created artifacts
echo "GitHub release artifacts:"
ls -l -R ./artifacts/github

echo "Upload artifacts:"
ls -l -R ./artifacts/upload 
