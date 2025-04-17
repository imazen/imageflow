#!/bin/bash
set -e # Exit on failure.
set -o pipefail # Ensure pipeline failures are caught.

# Usage: ./pack_native_dotnet.sh [tool]
# Requires Environment Variables:
# - PACKAGE_SUFFIX: The suffix for the package name (e.g., win-x64)
# - NUGET_RUNTIME: The .NET Runtime Identifier (RID) (e.g., win-x64)
# - CI_TAG: The Git tag (e.g., v1.2.3) used to derive NUGET_PACKAGE_VERSION
# - REPO_NAME: The GitHub repository name (e.g., imazen/imageflow)
# - REL_BINARIES_DIR: Relative path to the directory containing pre-built binaries
# - REL_NUGET_OUTPUT_DIR: Relative path to the directory where the .nupkg should be saved
# - REL_NUGET_ARCHIVE_DIR (Optional): Relative path to an archive directory

echo "Running pack_native_dotnet.sh from $(pwd)"

# --------------------------------------------------------------------------------
# Source Utilities & Validate Inputs
# --------------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Source utils *once* at the top
source "${SCRIPT_DIR}/utils.sh"

PACKAGE_TYPE="runtime"
if [[ "$1" == "tool" ]]; then
    PACKAGE_TYPE="tool"
    echo "Package type: tool"
else
    echo "Package type: runtime"
fi

validate_env_var() {
    local var_name="$1"
    if [[ -z "${!var_name}" ]]; then
        echo "Error: Required environment variable ${var_name} is not set."
        exit 1
    fi
    if [[ "$2" == "dir" ]] && [[ ! -d "${!var_name}" ]]; then
         echo "Error: Directory specified by ${var_name} does not exist: ${!var_name}"
         exit 1
    fi
    if [[ "$3" == "slash" ]] && [[ "${!var_name: -1}" != "/" ]]; then
        echo "Error: Directory path ${var_name} must end with a slash: ${!var_name}"
        exit 1
    fi
}

validate_env_var "PACKAGE_SUFFIX"
validate_env_var "NUGET_RUNTIME"
validate_env_var "CI_TAG"
validate_env_var "REPO_NAME"
validate_env_var "NUGET_PACKAGE_VERSION"
validate_env_var "REL_BINARIES_DIR" "dir" "slash"
validate_env_var "REL_NUGET_OUTPUT_DIR" "" "slash" # Don't check existence yet, script creates it

# --------------------------------------------------------------------------------
# Determine Package Details
# --------------------------------------------------------------------------------
if [[ "$PACKAGE_TYPE" == "tool" ]]; then
    export NUGET_PACKAGE_NAME="Imageflow.NativeTool.${PACKAGE_SUFFIX}"
    export PACKAGE_DESCRIPTION="imageflow_tool is a fast, correct image processing tool suitable for web servers. This package contains only the native executable; see Imageflow.Net for a managed wrapper."
    CSPROJ_TEMPLATE="${SCRIPT_DIR}/templates/tool.csproj.template"
else
    export NUGET_PACKAGE_NAME="Imageflow.NativeRuntime.${PACKAGE_SUFFIX}"
    export PACKAGE_DESCRIPTION="Imageflow is a fast, server-side-safe, and correct image processing library written in Rust. This package contains only the native library; use 'Imageflow.Net' or 'Imageflow.AllPlatforms' for the managed wrapper."
    CSPROJ_TEMPLATE="${SCRIPT_DIR}/templates/runtime.csproj.template"
fi

# NUGET_PACKAGE_VERSION is now expected to be set by the calling environment (workflow/test script)
echo "Using NUGET_PACKAGE_VERSION: ${NUGET_PACKAGE_VERSION}"

# Determine platform-specific binary names
NATIVE_BINARY_NAME=""
LIB_NAME="" # Only used for runtime packages
TOOL_NAME=""
if [[ "${NUGET_RUNTIME}" == *'win'* ]]; then
    LIB_NAME="imageflow.dll"
    TOOL_NAME="imageflow_tool.exe"
elif [[ "${NUGET_RUNTIME}" == *'osx'* ]]; then
    LIB_NAME="libimageflow.dylib"
    TOOL_NAME="imageflow_tool"
else # Linux, MUSL
    LIB_NAME="libimageflow.so"
    TOOL_NAME="imageflow_tool"
fi

if [[ "$PACKAGE_TYPE" == "tool" ]]; then
    NATIVE_BINARY_NAME="$TOOL_NAME"
else
    NATIVE_BINARY_NAME="$LIB_NAME"
fi

# Check if template exists
if [[ ! -f "$CSPROJ_TEMPLATE" ]]; then
    echo "Error: Project template not found: $CSPROJ_TEMPLATE"
    exit 1
fi

# --------------------------------------------------------------------------------
# Resolve Paths & Check Binaries
# --------------------------------------------------------------------------------
REPO_ROOT=$(resolve_path "${SCRIPT_DIR}/../..")
BINARIES_DIR="${REPO_ROOT}/${REL_BINARIES_DIR}"
NUGET_OUTPUT_DIR="${REPO_ROOT}/${REL_NUGET_OUTPUT_DIR}"
NUGET_ARCHIVE_DIR=""
if [[ -n "$REL_NUGET_ARCHIVE_DIR" ]]; then
    NUGET_ARCHIVE_DIR="${REPO_ROOT}/${REL_NUGET_ARCHIVE_DIR}"
fi

# Check that the required source binary exists
BINARY_PATH="${BINARIES_DIR}${NATIVE_BINARY_NAME}"
if [[ ! -f "$BINARY_PATH" ]]; then
    # Handle MUSL case: static library (.a) doesn't produce a runtime package
    if [[ "$PACKAGE_TYPE" == "runtime" && "$NUGET_RUNTIME" == *"musl"* && -f "${BINARIES_DIR}libimageflow.a" ]]; then
        echo "Skipping MUSL runtime package for ${PACKAGE_SUFFIX} as only static libimageflow.a exists."
        exit 0
    fi
    echo "Error: Required binary not found: $BINARY_PATH"
    exit 1
fi

# --------------------------------------------------------------------------------
# Prepare Staging Area & Pack
# --------------------------------------------------------------------------------
export NUGET_COMBINED_NAME="${NUGET_PACKAGE_NAME}.${NUGET_PACKAGE_VERSION}"

# Use helper function to create staging dir and setup trap
# Run main logic in a subshell to isolate trap
(
    STAGING_DIR=$(create_staging_dir "${SCRIPT_DIR}/staging" "native_${PACKAGE_TYPE}_${PACKAGE_SUFFIX}") || exit 1
    TEMP_PACKAGE_DIR="${STAGING_DIR}/package_output" # Temp dir for dotnet pack output
    mkdir -p "$TEMP_PACKAGE_DIR"

    # Enter staging directory for relative path operations
    cd "$STAGING_DIR" || exit 1

    echo "Preparing files in staging directory..."

    # 1. Copy and rename csproj template
    if [[ ! -f "$CSPROJ_TEMPLATE" ]]; then # Check template existence again just before use
        echo "Error: Project template not found: $CSPROJ_TEMPLATE"
        exit 1
    fi
    cp "$CSPROJ_TEMPLATE" ./project.csproj

    # 2. Create structure and copy native binary
    RUNTIME_TARGET_DIR="runtimes/${NUGET_RUNTIME}/native/"
    mkdir -p "$RUNTIME_TARGET_DIR"
    # Check source binary again just before copy
    if [[ ! -f "$BINARY_PATH" ]]; then
        echo "Error: Required binary not found just before copy: $BINARY_PATH"
        exit 1
    fi
    cp "$BINARY_PATH" "${RUNTIME_TARGET_DIR}${NATIVE_BINARY_NAME}"
    echo "Copied binary: ${NATIVE_BINARY_NAME}"

    # 3. Copy common files (README, LICENSE, icon)
    cp "${SCRIPT_DIR}/README.md" .
    cp "${SCRIPT_DIR}/LICENSE.md" .
    cp "${SCRIPT_DIR}/icon.png" .
    echo "Copied README.md, LICENSE.md, icon.png"

    # 4. Handle .targets files (runtime packages, Windows x86/x64 only)
    INCLUDE_TARGETS="false"
    if [[ "$PACKAGE_TYPE" == "runtime" && ("${NUGET_RUNTIME}" == "win-x64" || "${NUGET_RUNTIME}" == "win-x86") ]]; then
        INCLUDE_TARGETS="true"
        ARCH="${NUGET_RUNTIME#win-}" # x64 or x86
        TARGETS_TEMPLATE="${SCRIPT_DIR}/templates/imageflow_${ARCH}.targets.template"
        if [[ ! -f "$TARGETS_TEMPLATE" ]]; then
            echo "Error: Targets template not found: $TARGETS_TEMPLATE"
            exit 1
        fi
        TARGETS_DEST_FILENAME="${NUGET_PACKAGE_NAME}.targets"
        mkdir -p build/net45
        mkdir -p buildTransitive/net45
        cp "$TARGETS_TEMPLATE" "build/net45/${TARGETS_DEST_FILENAME}"
        cp "$TARGETS_TEMPLATE" "buildTransitive/net45/${TARGETS_DEST_FILENAME}"
        echo "Copied .targets file for ${ARCH}"
    fi

    # 5. Handle special case for win-arm64 runtime (dummy lib file)
    if [[ "$PACKAGE_TYPE" == "runtime" && "$NUGET_RUNTIME" == "win-arm64" ]]; then
        mkdir -p lib/netstandard1.0
        touch lib/netstandard1.0/_._
        echo "Created dummy file for win-arm64"
    fi

    # --------------------------------------------------------------------------------
    # Build dotnet pack command arguments
    # --------------------------------------------------------------------------------
    PACK_ARGS=()
    # Note: --output is handled by run_dotnet_pack function
    PACK_ARGS+=("--configuration" "Release")
    # --no-build and --no-restore are handled by run_dotnet_pack
    PACK_ARGS+=("/p:PackageId=${NUGET_PACKAGE_NAME}")
    PACK_ARGS+=("/p:Version=${NUGET_PACKAGE_VERSION}")
    PACK_ARGS+=("/p:PackageDescription=\"${PACKAGE_DESCRIPTION}\"") # Quotes needed if description has spaces
    PACK_ARGS+=("/p:RepositoryUrl=https://github.com/${REPO_NAME}")
    PACK_ARGS+=("/p:ImageflowNugetRid=${NUGET_RUNTIME}")
    PACK_ARGS+=("/p:NativeBinaryName=${NATIVE_BINARY_NAME}")
    PACK_ARGS+=("/p:IncludeTargets=${INCLUDE_TARGETS}")
    # Add NoWarn for NU5128 (content file warning that doesn't apply to native packages)
    PACK_ARGS+=("/p:NoWarn=NU5128")

    echo "----------------------------------------"
    echo "Calculated pack arguments:"
    printf '%s\n' "${PACK_ARGS[@]}"
    echo "----------------------------------------"
    echo "Files staged for pack:"
    find . -type f -printf '%P\n' || find . -type f # osx doesn't support -printf
    echo "----------------------------------------"

    # --------------------------------------------------------------------------------
    # Execute dotnet pack using helper function
    # --------------------------------------------------------------------------------
    FINAL_NUPKG_PATH=$(run_dotnet_pack "${STAGING_DIR}/project.csproj" "${NUGET_PACKAGE_NAME}" "${NUGET_PACKAGE_VERSION}" "${TEMP_PACKAGE_DIR}" "${NUGET_OUTPUT_DIR}" "${NUGET_ARCHIVE_DIR}" "${PACK_ARGS[@]}") || exit 1

    # --------------------------------------------------------------------------------
    # Verify Package using helper function
    # --------------------------------------------------------------------------------
    echo "Verifying package contents using helper script..."
    # For native packages, we don't compare against a gold nuspec currently
    run_verify_script "${FINAL_NUPKG_PATH}" "" || exit 1

) # Exit staging subshell

exit_code=$?
if [[ $exit_code -ne 0 ]]; then
    echo "Error: Failed to pack native package ${NUGET_PACKAGE_NAME} in subshell."
    exit $exit_code
fi

echo "Successfully packed and verified ${NUGET_PACKAGE_NAME}"
