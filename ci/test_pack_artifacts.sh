#!/bin/bash
set -euo pipefail

# ------------------------------------------------------------------------------
# Save the original working directory and create a unique temporary directory
# for the entire test run. Each matrix variant will have its own subdirectory.
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
# Determine the repository root relative to this script.
# Assumes that test_pack_artifacts.sh lives in the ci/ directory.
# ------------------------------------------------------------------------------
repo_root=$(cd "$(dirname "$0")/.." && pwd)
echo "Repository root: $repo_root"

# ------------------------------------------------------------------------------
# Define all matrix target variants to test.
#
#   • linux: dynamic library 'libimageflow.so', static library 'libimageflow.a',
#            and tool binary 'imageflow_tool'
#   • win:   dynamic library 'imageflow.dll', static library 'imageflow.lib',
#            and tool binary 'imageflow_tool.exe'
#   • apple: dynamic library 'libimageflow.dylib', static library 'libimageflow.a',
#            and tool binary 'imageflow_tool'
#   • musl:  only static library 'libimageflow.a' and tool binary 'imageflow_tool'
# ------------------------------------------------------------------------------
targets=("linux" "win" "apple" "musl")

for target in "${targets[@]}"; do
    echo "-----------------------------------------"
    echo "Testing matrix variant: $target"

    # Create a unique subdirectory for this variant
    variant_dir="${temp_dir}/test_${target}"
    mkdir -p "$variant_dir"
    pushd "$variant_dir" > /dev/null

    # ------------------------------------------------------------------------------
    # Create the minimal repository structure needed by pack_artifacts.sh:
    #   - ci/packaging_extras for install/uninstall scripts.
    #   - bindings/headers for header files.
    #   - rel_binaries for release binaries.
    #   - build_artifacts/doc for documentation.
    # ------------------------------------------------------------------------------
    mkdir -p ci/packaging_extras bindings/headers rel_binaries build_artifacts/doc

    # Copy the live pack_artifacts.sh from the repository to avoid embedding its code.
    cp "$repo_root/ci/pack_artifacts.sh" ci/pack_artifacts.sh
    chmod +x ci/pack_artifacts.sh

    # ------------------------------------------------------------------------------
    # Create dummy installation scripts required by pack_artifacts.sh.
    # ------------------------------------------------------------------------------
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

    # ------------------------------------------------------------------------------
    # Create dummy header files.
    # ------------------------------------------------------------------------------
    cat << EOF > bindings/headers/imageflow_default.h
/* Dummy imageflow_default.h header for ${target} */
EOF


    # ------------------------------------------------------------------------------
    # Create dummy binaries in rel_binaries based on the target.
    #
    # The expected file names come from the CI matrix (see .github/workflows/ci.yml):
    #   - On win:
    #         LIBIMAGEFLOW_DYNAMIC="imageflow.dll"
    #         LIBIMAGEFLOW_STATIC="imageflow.lib"
    #         IMAGEFLOW_TOOL="imageflow_tool.exe"
    #
    #   - On apple:
    #         LIBIMAGEFLOW_DYNAMIC="libimageflow.dylib"
    #         LIBIMAGEFLOW_STATIC="libimageflow.a"
    #         IMAGEFLOW_TOOL="imageflow_tool"
    #
    #   - On linux:
    #         LIBIMAGEFLOW_DYNAMIC="libimageflow.so"
    #         LIBIMAGEFLOW_STATIC="libimageflow.a"
    #         IMAGEFLOW_TOOL="imageflow_tool"
    #
    #   - On musl:
    #         Only the static library file is created
    #         IMAGEFLOW_TOOL is still created as "imageflow_tool"
    # ------------------------------------------------------------------------------
    case "$target" in
        win)
            export LIBIMAGEFLOW_DYNAMIC="imageflow.dll"
            export LIBIMAGEFLOW_STATIC="imageflow.lib"
            export IMAGEFLOW_TOOL="imageflow_tool.exe"
            touch rel_binaries/imageflow.dll
            touch rel_binaries/imageflow.lib
            touch rel_binaries/imageflow_tool.exe
            ;;
        apple)
            export LIBIMAGEFLOW_DYNAMIC="libimageflow.dylib"
            export LIBIMAGEFLOW_STATIC="libimageflow.a"
            export IMAGEFLOW_TOOL="imageflow_tool"
            touch rel_binaries/libimageflow.dylib
            touch rel_binaries/libimageflow.a
            touch rel_binaries/imageflow_tool
            ;;
        linux)
            export LIBIMAGEFLOW_DYNAMIC="libimageflow.so"
            export LIBIMAGEFLOW_STATIC="libimageflow.a"
            export IMAGEFLOW_TOOL="imageflow_tool"
            touch rel_binaries/libimageflow.so
            touch rel_binaries/libimageflow.a
            touch rel_binaries/imageflow_tool
            ;;
        musl)
            # On musl, only a static library is produced.
            export LIBIMAGEFLOW_DYNAMIC="libimageflow.so"
            export LIBIMAGEFLOW_STATIC="libimageflow.a"
            export IMAGEFLOW_TOOL="imageflow_tool"
            touch rel_binaries/libimageflow.a
            touch rel_binaries/imageflow_tool
            ;;
    esac

    # ------------------------------------------------------------------------------
    # Create dummy documentation to trigger docs packaging.
    # ------------------------------------------------------------------------------
    echo "Dummy documentation content for ${target}" > build_artifacts/doc/doc.txt

    # ------------------------------------------------------------------------------
    # Export environment variables to mimic the CI matrix expansion.
    # ------------------------------------------------------------------------------
    export TARGET_DIR="build_artifacts/"
    export REL_BINARIES_DIR="rel_binaries/"
    export EXTENSION="tar.gz"
    export IMAGEFLOW_TAG_SHA_SUFFIX="testtagsha-${target}"
    export GITHUB_SHA="abcdef123456"
    export GITHUB_REF_NAME="v1.0-${target}"
    export MATRIX_COMMIT_SUFFIX="commit123-${target}"
    export HTTPS_UPLOAD_BASE="https://uploads.example.com"
    export MATRIX_TARGET="$target"

    echo "Environment for target $target:"
    env | grep -E '^(TARGET_DIR|REL_BINARIES_DIR|EXTENSION|IMAGEFLOW_TAG_SHA_SUFFIX|LIBIMAGEFLOW_DYNAMIC|LIBIMAGEFLOW_STATIC|IMAGEFLOW_TOOL|GITHUB_SHA|GITHUB_REF_NAME|MATRIX_COMMIT_SUFFIX|HTTPS_UPLOAD_BASE|MATRIX_TARGET)='

    # ------------------------------------------------------------------------------
    # Run the packaging script using the mocked inputs.
    # ------------------------------------------------------------------------------
    echo "Running pack_artifacts.sh for target $target..."
    bash ci/pack_artifacts.sh

    # ------------------------------------------------------------------------------
    # List and display all created artifact files for verification.
    # ------------------------------------------------------------------------------
    echo "Artifacts created for target $target:"
    find artifacts -type f

    popd > /dev/null
done

echo "All matrix variant tests completed successfully." 
