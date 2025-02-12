#!/bin/bash
set -e #Exit on failure.
set -o pipefail 
# REQUIRES PACKAGE_SUFFIX
# REQUIRES NUGET_RUNTIME
# REQUIRES CI_TAG
# REQUIRES REPO_NAME
# REQUIRES BINARIES_DIR

echo "Running pack.sh from $(pwd)"

source "$(dirname "${BASH_SOURCE[0]}")/utils.sh"


if [[ -z "$PACKAGE_SUFFIX" ]]; then
    echo "PACKAGE_SUFFIX not set. Should be the package suffix (like win-x64) for producing Imageflow.NativeRuntime.win-x64"
    exit 1
fi

if [[ "$1" == "tool" ]]; then
    export NUGET_PACKAGE_NAME=Imageflow.NativeTool.${PACKAGE_SUFFIX}
    export PACKAGE_DESCRIPTION="imageflow_tool is a fast, correct image processing tool suitable for web servers. This package contains only the native executable; see Imageflow.Net for a managed wrapper."
else
    export NUGET_PACKAGE_NAME=Imageflow.NativeRuntime.${PACKAGE_SUFFIX}
    export PACKAGE_DESCRIPTION="Imageflow is a fast, server-side-safe, and correct image processing library written in Rust. This package contains only the native library; use 'Imageflow.Net' or 'Imageflow.AllPlatforms' for the managed wrapper."
fi

if [[ "$CI_TAG" == 'v'* ]]; then
    export NUGET_PACKAGE_VERSION="${CI_TAG#"v"}"
else
    echo "CI_TAG not set; skipping nuget package upload"
    exit 0
fi

# Validate required environment variables
if [[ -z "$REL_BINARIES_DIR" ]]; then
    echo "REL_BINARIES_DIR not set. Should be the location of imageflow.dll and imageflow_tool.exe"
    exit 1
fi

if [[ "${REL_BINARIES_DIR: -1}" != "/" ]]; then
    echo "REL_BINARIES_DIR must end with a slash: $REL_BINARIES_DIR"
    exit 1
fi
## Check existince of REL_BINARIES_DIR
if [[ ! -d "$REL_BINARIES_DIR" ]]; then
    echo "REL_BINARIES_DIR does not exist: $REL_BINARIES_DIR"
    exit 1
fi

# REL_NUGET_OUTPUT_DIR
if [[ -z "$REL_NUGET_OUTPUT_DIR" ]]; then
    echo "REL_NUGET_OUTPUT_DIR not set. Should be the location of the nuget output directory"
    exit 1
fi

if [[ "${REL_NUGET_OUTPUT_DIR: -1}" != "/" ]]; then
    echo "REL_NUGET_OUTPUT_DIR must end with a slash: $REL_NUGET_OUTPUT_DIR"
    exit 1
fi
## Check existince of REL_NUGET_OUTPUT_DIR
if [[ ! -d "$REL_NUGET_OUTPUT_DIR" ]]; then
    echo "REL_NUGET_OUTPUT_DIR does not exist: $REL_NUGET_OUTPUT_DIR"
    exit 1
fi

# REPO_NAME
if [[ -z "$REPO_NAME" ]]; then
    echo "REPO_NAME not set. Should be the name of the repository"
    exit 1
fi

if [[ -z "$NUGET_RUNTIME" ]]; then
    echo "NUGET_RUNTIME not set. Should be the RID to build for"
    exit 1
fi

# Resolve paths
SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}" )"
SCRIPT_DIR="$(cd "$SCRIPT_DIR"; pwd)"
BINARIES_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.."; pwd)/$REL_BINARIES_DIR


if [[ ! -d "$BINARIES_DIR" ]]; then
    echo "BINARIES_DIR is not a directory: $BINARIES_DIR"
    exit 1
fi

# if $1 is not tool, and runtime contains musl, and libimageflow.so does not exist, skip this package
if [[ "$1" != "tool" && "$NUGET_RUNTIME" == *"musl"* && ! -f "$BINARIES_DIR/libimageflow.so" ]]; then
    echo "Skipping musl package for ${PACKAGE_SUFFIX}, libimageflow.so does not exist"
    exit 0
fi


export NUGET_COMBINED_NAME="$NUGET_PACKAGE_NAME.$NUGET_PACKAGE_VERSION"

RANDOM_DIR_NAME=$(date +%s)_$RANDOM
STAGING_DIR="${SCRIPT_DIR}/staging/${RANDOM_DIR_NAME}"
mkdir -p "$STAGING_DIR" || true

( cd "$STAGING_DIR"
    rm -rf "./$NUGET_COMBINED_NAME"
    mkdir "$NUGET_COMBINED_NAME"
    cd "$NUGET_COMBINED_NAME"
    
    RUNTIME_DIR="runtimes/${NUGET_RUNTIME}/native/"
    PROPS_PATH="build/net45/${NUGET_PACKAGE_NAME}.targets"
    PROPS_PATH_2="buildTransitive/net45/${NUGET_PACKAGE_NAME}.targets"
    NUGET_OUTPUT_DIR="${SCRIPT_DIR}/../../${REL_NUGET_OUTPUT_DIR}"
    NUGET_OUTPUT_FILE="${NUGET_OUTPUT_DIR}${NUGET_COMBINED_NAME}.nupkg"
  
    mkdir -p "${NUGET_OUTPUT_DIR}" || true
    
    # Set up platform-specific files
    if [[ "${NUGET_RUNTIME}" == *'win'* ]]; then
        LIB_NAME=imageflow.dll
        TOOL_NAME=imageflow_tool.exe
    elif [[ "${NUGET_RUNTIME}" == *'osx'* ]]; then
        LIB_NAME=libimageflow.dylib
        TOOL_NAME=imageflow_tool
    else
        LIB_NAME=libimageflow.so
        TOOL_NAME=imageflow_tool
    fi

    # Copy binaries
    mkdir -p "$RUNTIME_DIR"
    if [[ "$1" == "tool" ]]; then
        cp "${BINARIES_DIR//\\//}${TOOL_NAME}" "${RUNTIME_DIR}${TOOL_NAME}"
    else
        cp "${BINARIES_DIR//\\//}${LIB_NAME}" "${RUNTIME_DIR}${LIB_NAME}"
    fi
    
    # Handle special case for win-arm64
    
    if [[ "${NUGET_RUNTIME}" == "win-arm64" ]]; then
        mkdir -p lib/netstandard1.0
        echo "" > lib/netstandard1.0/_._
        DEPENDENCIES='    <dependencies>
      <group targetFramework=".NETStandard1.0" />
    </dependencies>'
    else 
        DEPENDENCIES="<dependencies></dependencies>"
    fi

    # Set up Windows-specific props files
    if [[ "${NUGET_RUNTIME}" == *'win'* ]] && [[ "$1" != "tool" ]]; then
        if [[ "${NUGET_RUNTIME}" == *'x64'* ]] || [[ "${NUGET_RUNTIME}" == *'x86'* ]]; then
            mkdir -p build/net45
            mkdir -p buildTransitive/net45
            ARCH="${NUGET_RUNTIME#*-}"
            cat "${SCRIPT_DIR}/imageflow_${ARCH}.targets" | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
            cat "${SCRIPT_DIR}/imageflow_${ARCH}.targets" | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH_2"
        fi
        if [[ "${NUGET_RUNTIME}" == *'arm64'* ]]; then
            echo "Skipping .NET Framework 4.x compat for win-arm64, xplat nuget is fully broken for win-arm64"
        fi 
    fi


    PACKAGE_DESCRIPTION="$(echo $PACKAGE_DESCRIPTION | sed -e 's/[\/&]/\\&/g')"
    DEPENDENCIES="$(echo $DEPENDENCIES | sed -e 's/[\/&]/\\&/g')"
    #if REPO_NAME contains \, fail
    if [[ "$REPO_NAME" == *"\\"* ]]; then
        echo "REPO_NAME contains a backslash: $REPO_NAME"
        exit 1
    fi
    SED_REPO_NAME="$(echo $REPO_NAME | sed -e 's/\//\\\//g')"

    # Create nuspec from template   
    echo "Modifying template to create ${NUGET_PACKAGE_NAME}.nuspec, using values: ${NUGET_PACKAGE_NAME}, ${NUGET_PACKAGE_VERSION}, ${SED_REPO_NAME}, ${PACKAGE_DESCRIPTION}, ${DEPENDENCIES}"
    # Check none are empty
    if [[ -z "$NUGET_PACKAGE_NAME" || -z "$NUGET_PACKAGE_VERSION" || -z "$SED_REPO_NAME" || -z "$PACKAGE_DESCRIPTION" || -z "$DEPENDENCIES" ]]; then
        echo "One or more variables are empty"
        exit 1
    fi

    NUSPEC_NAME="${NUGET_PACKAGE_NAME}.nuspec"
    
    
    cat "${SCRIPT_DIR}/native_template.nuspec" \
    | sed -e "s/:id:/${NUGET_PACKAGE_NAME}/g" \
    | sed -e "s/:version:/${NUGET_PACKAGE_VERSION}/g" \
    > "${NUSPEC_NAME}.temp1" \
      || { echo "Failed to inject id and version (${NUGET_PACKAGE_NAME}, ${NUGET_PACKAGE_VERSION})"; exit 1; }

    cat "${NUSPEC_NAME}.temp1" | sed -e "s/:repo_name:/${SED_REPO_NAME}/g" > "${NUSPEC_NAME}.temp2" \
      || { echo "Failed to inject repo_name (${REPO_NAME} -> ${SED_REPO_NAME})"; exit 1; }
    cat "${NUSPEC_NAME}.temp2" | sed -e "s/:package_description:/${PACKAGE_DESCRIPTION}/g" > "${NUSPEC_NAME}.temp3" \
      || { echo "Failed to inject package_description (${PACKAGE_DESCRIPTION})"; exit 1; }
    cat "${NUSPEC_NAME}.temp3" | sed -e "s/:dependencies:/${DEPENDENCIES}/g" > "${NUSPEC_NAME}.temp4" \
      || { echo "Failed to inject dependencies (${DEPENDENCIES})"; exit 1; }

    mv "${NUSPEC_NAME}.temp4" "${NUSPEC_NAME}"
    rm "${NUSPEC_NAME}.temp1" "${NUSPEC_NAME}.temp2" "${NUSPEC_NAME}.temp3"
    
    echo "${NUSPEC_NAME}:"
    cat "${NUSPEC_NAME}"
    echo "----------------------------------------"
    # Check if the nuspec file was created and is not empty
    if [[ ! -f "${NUSPEC_NAME}" || ! -s "${NUSPEC_NAME}" ]]; then
        echo "Failed to create ${NUSPEC_NAME} using sed, empty or missing"
        exit 1
    fi  

    # Copy package metadata files
    mkdir -p _rels
    cat "${SCRIPT_DIR}/.rels" | sed -e "s/:nuspec_name:/${NUGET_PACKAGE_NAME}.nuspec/g" > "_rels/.rels"
    cp "${SCRIPT_DIR}/[Content_Types].xml" .
    
    # Copy documentation
    cp "${SCRIPT_DIR}/README.md" .
    cp "${SCRIPT_DIR}/LICENSE.md" .
    cp "${SCRIPT_DIR}/icon.png" .


    # Create package with multiple fallbacks
    if [[ -f "${NUGET_OUTPUT_FILE}" ]]; then
        rm "${NUGET_OUTPUT_FILE}" || true
    fi

    echo "Packing ${NUGET_OUTPUT_FILE} with the following files:"
    # (relative only, short paths, not grouped by directory):"
    echo "----------------------------------------"
    find . -type f -printf '%P\n' || find . -type f #osx doesn't support -printf
    echo "----------------------------------------"
    
    echo "Attempting to create package..."
    if ! create_package "${NUGET_OUTPUT_FILE}" "$STAGING_DIR"; then
        echo "Failed to create package ${PACKAGE_NAME}"
        exit 1
    fi
    
    echo "${NUGET_OUTPUT_FILE} packed successfully"

    # if REL_NUGET_ARCHIVE_DIR is defined, copy the package to it
    if [[ -n "$REL_NUGET_ARCHIVE_DIR" ]]; then
        echo "Copying ${NUGET_OUTPUT_FILE} to ${REL_NUGET_ARCHIVE_DIR}"
        mkdir -p "${SCRIPT_DIR}/../../${REL_NUGET_ARCHIVE_DIR}"
        cp "${NUGET_OUTPUT_FILE}" "${SCRIPT_DIR}/../../${REL_NUGET_ARCHIVE_DIR}"
    fi
)
