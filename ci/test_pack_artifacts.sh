#!/bin/bash
set -euo pipefail

# ------------------------------------------------------------------------------
# Save the current directory and create a unique temporary directory for all matrix tests.
# ------------------------------------------------------------------------------
orig_dir=$(pwd)
temp_dir=$(mktemp -d)
echo "Created main temporary directory: $temp_dir"

cleanup() {
    echo "Cleaning up temporary directory..."
    rm -rf "$temp_dir"
    cd "$orig_dir"
}
trap cleanup EXIT

# ------------------------------------------------------------------------------
# Determine repository root (assumes this script is in ci/)
# ------------------------------------------------------------------------------
repo_root=$(cd "$(dirname "$0")/.." && pwd)
echo "Repository root: $repo_root"

# ------------------------------------------------------------------------------
# Global CI variables (simulate GitHub env vars)
# ------------------------------------------------------------------------------
export PROFILE="release"
export GITHUB_REF_NAME="v1.0.0-rc01"
export GITHUB_SHA_SHORT="abcdef1"
export GITHUB_SHA="abcdef1234567890"
export HTTPS_UPLOAD_BASE="https://s3.us-west-1.amazonaws.com/imageflow-nightlies"

# ------------------------------------------------------------------------------
# Define all matrix variants as a list with fields:
# matrix_name | suffix | commit_suffix | target | os | static
#
# These values mirror the YAML matrix:
#
# 1. linux-x64:
#      suffix: ubuntu-x86_64, commit_suffix: linux-x64, target: x86_64-unknown-linux-gnu, os: ubuntu-20.04
# 2. linux-x64-ubuntu-24:
#      suffix: ubuntu-x86_64-24, commit_suffix: linux64_24, target: x86_64-unknown-linux-gnu, os: ubuntu-24.04
# 3. linux-arm64-ubuntu-22:
#      suffix: ubuntu-arm64, commit_suffix: linux-arm64, target: aarch64-unknown-linux-gnu, os: ubuntu-22-arm-16gb
# 4. osx-x64-13:
#      suffix: osx-x86_64, commit_suffix: mac-x64, target: x86_64-apple-darwin, os: macos-13
# 5. osx-arm64-14:
#      suffix: osx-arm64, commit_suffix: mac-arm64, target: aarch64-apple-darwin, os: macos-14
# 6. win-x64-2022:
#      suffix: win-x86_64, commit_suffix: win-x64, target: x86_64-pc-windows-msvc, os: windows-2022
# 7. win-x86-2022:
#      suffix: win-x86, commit_suffix: win-x86, target: i686-pc-windows-msvc, os: windows-2022
# 8. win-arm64-11:
#      suffix: win-arm64, commit_suffix: win-arm64, target: aarch64-pc-windows-msvc, os: windows-11-arm-16gb
# 9. linux-musl-x64:
#      suffix: linux-musl-x64, commit_suffix: linux-musl-x64, target: x86_64-unknown-linux-musl, os: ubuntu-24.04, static: true
# 10. linux-musl-arm64:
#      suffix: linux-musl-arm64, commit_suffix: linux-musl-arm64, target: aarch64-unknown-linux-musl, os: ubuntu-22-arm-16gb, static: true
# ------------------------------------------------------------------------------
matrices=(
  "linux-x64|ubuntu-x86_64|linux64|x86_64-unknown-linux-gnu|ubuntu-20.04|false"
  "linux-x64-ubuntu-24|ubuntu-x86_64-24|linux64_24|x86_64-unknown-linux-gnu|ubuntu-24.04|false"
  "linux-arm64-ubuntu-22|ubuntu-arm64|linux-arm64|aarch64-unknown-linux-gnu|ubuntu-22-arm-16gb|false"
  "osx-x64-13|osx-x86_64|osx-x64|x86_64-apple-darwin|macos-13|false"
  "osx-arm64-14|osx-arm64|osx-arm64|aarch64-apple-darwin|macos-14|false"
  "win-x64-2022|win-x86_64|win-x64|x86_64-pc-windows-msvc|windows-2022|false"
  "win-x86-2022|win-x86|win-x86|i686-pc-windows-msvc|windows-2022|false"
  "win-arm64-11|win-arm64|win-arm64|aarch64-pc-windows-msvc|windows-11-arm-16gb|false"
  "linux-musl-x64|linux-musl-x64|linux-musl-x64|x86_64-unknown-linux-musl|ubuntu-24.04|true"
  "linux-musl-arm64|linux-musl-arm64|linux-musl-arm64|aarch64-unknown-linux-musl|ubuntu-22-arm-16gb|true"
)

# ------------------------------------------------------------------------------
# Loop through each matrix variant and perform a packaging test.
# ------------------------------------------------------------------------------
for matrix in "${matrices[@]}"; do
    IFS="|" read -r matrix_name suffix commit_suffix target os static_flag <<< "$matrix"
    echo "========================================="
    echo "Testing matrix variant: $matrix_name"
    
    # Create a sandbox subdirectory for this test variant.
    variant_dir="${temp_dir}/test_${matrix_name}"
    mkdir -p "$variant_dir"
    pushd "$variant_dir" > /dev/null

    # --------------------------------------------------------------------------
    # Set up CI-like environment variables (as in the YAML 'Set env vars' steps).
    # --------------------------------------------------------------------------
    export TARGET_DIR="target/${target}/"
    export TAG_SHA_SUFFIX="${GITHUB_REF_NAME}-${GITHUB_SHA_SHORT}-${suffix}"
    export REL_BINARIES_DIR="${TARGET_DIR}${PROFILE}/"
    export IMAGEFLOW_TAG_SHA_SUFFIX="imageflow-${TAG_SHA_SUFFIX}"
    export MATRIX_COMMIT_SUFFIX="${commit_suffix}"
    export MATRIX_TARGET="${target}"
    export TAG_SHA_SUFFIX  # (already set above)

    # Determine LIBIMAGEFLOW_DYNAMIC based on matrix.target.
    if [[ "${target}" == *"win"* ]]; then
        export LIBIMAGEFLOW_DYNAMIC="imageflow.dll"
    elif [[ "${target}" == *"apple"* ]]; then
        export LIBIMAGEFLOW_DYNAMIC="libimageflow.dylib"
    else
        export LIBIMAGEFLOW_DYNAMIC="libimageflow.so"
    fi

    # Determine LIBIMAGEFLOW_STATIC.
    if [[ "${target}" == *"win"* ]]; then
        export LIBIMAGEFLOW_STATIC="imageflow.lib"
    else
        export LIBIMAGEFLOW_STATIC="libimageflow.a"
    fi

    # Determine IMAGEFLOW_TOOL.
    if [[ "${target}" == *"win"* ]]; then
        export IMAGEFLOW_TOOL="imageflow_tool.exe"
    else
        export IMAGEFLOW_TOOL="imageflow_tool"
    fi

    # Determine EXTENSION based on the OS.
    if [[ "${os}" == *"windows"* ]]; then
        export EXTENSION="zip"
    else
        export EXTENSION="tar.gz"
    fi

    echo "Environment for $matrix_name:"
    echo "  TARGET_DIR                = ${TARGET_DIR}"
    echo "  REL_BINARIES_DIR          = ${REL_BINARIES_DIR}"
    echo "  TAG_SHA_SUFFIX            = ${TAG_SHA_SUFFIX}"
    echo "  IMAGEFLOW_TAG_SHA_SUFFIX  = ${IMAGEFLOW_TAG_SHA_SUFFIX}"
    echo "  MATRIX_COMMIT_SUFFIX      = ${MATRIX_COMMIT_SUFFIX}"
    echo "  MATRIX_TARGET             = ${MATRIX_TARGET}"
    echo "  LIBIMAGEFLOW_DYNAMIC      = ${LIBIMAGEFLOW_DYNAMIC}"
    echo "  LIBIMAGEFLOW_STATIC       = ${LIBIMAGEFLOW_STATIC}"
    echo "  IMAGEFLOW_TOOL            = ${IMAGEFLOW_TOOL}"
    echo "  EXTENSION                 = ${EXTENSION}"
    echo "  HTTPS_UPLOAD_BASE         = ${HTTPS_UPLOAD_BASE}"

    # --------------------------------------------------------------------------
    # Create the minimal repository structure required by pack_artifacts.sh.
    # --------------------------------------------------------------------------
    mkdir -p "${TARGET_DIR}${PROFILE}"
    mkdir -p build_artifacts/doc
    mkdir -p bindings/headers
    mkdir -p ci/packaging_extras

    # Copy the live pack_artifacts.sh from the repository.
    cp "$repo_root/ci/pack_artifacts.sh" ci/pack_artifacts.sh
    chmod +x ci/pack_artifacts.sh

    # Create dummy install/uninstall scripts.
    cat << 'EOF' > ci/packaging_extras/install.sh
#!/bin/bash
# Dummy install script for testing purposes
echo "Running dummy install script"
EOF
    chmod +x ci/packaging_extras/install.sh

    cat << 'EOF' > ci/packaging_extras/uninstall.sh
#!/bin/bash
# Dummy uninstall script for testing purposes
echo "Running dummy uninstall script"
EOF
    chmod +x ci/packaging_extras/uninstall.sh

    # Create a dummy header in bindings/headers.
    echo "/* Dummy header for ${matrix_name} */" > bindings/headers/imageflow_default.h

    # --------------------------------------------------------------------------
    # Create dummy binaries in the release directory.
    # For non-musl builds (static_flag false) create dynamic lib, static lib and tool.
    # For musl builds (static_flag true) only create static lib and tool.
    # --------------------------------------------------------------------------
    if [ "${static_flag}" == "true" ]; then
        touch "${REL_BINARIES_DIR}/${LIBIMAGEFLOW_STATIC}"
        touch "${REL_BINARIES_DIR}/${IMAGEFLOW_TOOL}"
    else
        touch "${REL_BINARIES_DIR}/${LIBIMAGEFLOW_DYNAMIC}"
        touch "${REL_BINARIES_DIR}/${LIBIMAGEFLOW_STATIC}"
        touch "${REL_BINARIES_DIR}/${IMAGEFLOW_TOOL}"
    fi

    # Create a dummy docs file.
    echo "Dummy documentation for ${matrix_name}" > build_artifacts/doc/doc.txt

    # --------------------------------------------------------------------------
    # Run the pack_artifacts.sh script (which should use the above env vars).
    # --------------------------------------------------------------------------
    echo "Running pack_artifacts.sh for matrix variant: ${matrix_name}..."
    bash ci/pack_artifacts.sh

    # List any produced artifact files for verification.
    echo "Artifacts created for ${matrix_name}:"
    if [ -d artifacts ]; then
      find artifacts -type f || true
    else
      echo "No artifacts directory found."
    fi

    popd > /dev/null
done

echo "All matrix variant tests completed successfully." 
