#!/bin/bash
set -e #Exit on failure.

# REQUIRES PACKAGE_SUFFIX
# REQUIRES NUGET_RUNTIME
# REQUIRES CI_TAG
# REQUIRES REPO_NAME
# REQUIRES BINARIES_DIR


if [[ -z "$PACKAGE_SUFFIX" ]]; then
    echo "PACKAGE_SUFFIX not set. Should be the package suffix (like win-x64) for producing Imageflow.NativeRuntime.win-x64"
    exit 1
fi

if [[ "$1" == "tool" ]]; then
    export NUGET_PACKAGE_NAME=Imageflow.NativeTool.${PACKAGE_SUFFIX}
else
    export NUGET_PACKAGE_NAME=Imageflow.NativeRuntime.${PACKAGE_SUFFIX}
fi


if [[ "$CI_TAG" == 'v'* ]]; then
    export NUGET_PACKAGE_VERSION="${CI_TAG#"v"}"
else
    echo "CI_TAG not set; skipping nuget package upload"
    exit 0
fi

# fail if any of these are not set
if [[ -z "$BINARIES_DIR" ]]; then
    echo "BINARIES_DIR not set. Should be the location of imageflow.dll and imageflow_tool.exe"
    exit 1
fi

# fail if BINARIES_DIR doesn't have a trailing slash
if [[ "${BINARIES_DIR: -1}" != "/" ]]; then
    echo "BINARIES_DIR must end with a slash: $BINARIES_DIR"
    exit 1
fi

if [[ -z "$REPO_NAME" ]]; then
    echo "REPO_NAME not set. Should be the name of the repository"
    exit 1
fi


if [[ -z "$NUGET_RUNTIME" ]]; then
    echo "NUGET_RUNTIME not set. Should be the RID to build for"
    exit 1
fi

# Resolve the relative path in BINARIES_DIR relative to the root of the repository (../../)
# We are currintly in ci/pack_nuget/pack.sh
BINARIES_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.."; pwd)/$BINARIES_DIR

# fail if BINARIES_DIR is not a directory
if [[ ! -d "$BINARIES_DIR" ]]; then
    echo "BINARIES_DIR is not a directory: $BINARIES_DIR"
    exit 1
fi


export NUGET_COMBINED_NAME="$NUGET_PACKAGE_NAME.$NUGET_PACKAGE_VERSION"

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}" )"
SCRIPT_DIR="$(cd "$SCRIPT_DIR"; pwd)"

STAGING_DIR="${SCRIPT_DIR}/staging"

mkdir -p "$STAGING_DIR" || true

( cd "$STAGING_DIR"
    
    
    rm -rf "./$NUGET_COMBINED_NAME"
    mkdir "$NUGET_COMBINED_NAME"
    cd "$NUGET_COMBINED_NAME"
    
    
    RUNTIME_DIR="runtimes/${NUGET_RUNTIME}/native/"
    PROPS_PATH="build/net45/${NUGET_PACKAGE_NAME}.targets"
    PROPS_PATH_2="buildTransitive/net45/${NUGET_PACKAGE_NAME}.targets"
    NUGET_OUTPUT_DIR="${SCRIPT_DIR}/../../artifacts/nuget"
    NUGET_OUTPUT_FILE="${NUGET_OUTPUT_DIR}/${NUGET_COMBINED_NAME}.nupkg"
  
    mkdir -p "${NUGET_OUTPUT_DIR}" || true
    
    
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
    
    mkdir -p lib/netstandard1.0
    echo "" > lib/netstandard1.0/_._
    
    mkdir -p "$RUNTIME_DIR"
    if [[ "$1" == "tool" ]]; then
        cp "${BINARIES_DIR//\\//}${TOOL_NAME}" "${RUNTIME_DIR}${TOOL_NAME}"
    else
        cp "${BINARIES_DIR//\\//}${LIB_NAME}" "${RUNTIME_DIR}${LIB_NAME}"
    fi
    
    
    SED_NUGET_PACKAGE_NAME="$(echo $NUGET_PACKAGE_NAME | sed -e 's/[\/&]/\\&/g')"
    SED_NUGET_PACKAGE_VERSION="$(echo $NUGET_PACKAGE_VERSION | sed -e 's/[\/&]/\\&/g')"
    SED_NUGET_LIBFILE="$(echo $RUNTIME_DIR$LIB_NAME | sed -e 's/[\/&]/\\&/g' | sed -e 's/\//\\/g')" # fix slashes too
    
    
    if [[ "${NUGET_RUNTIME}" == *'win'* ]]; then
        if [[ "${NUGET_RUNTIME}" == *'x64'* ]]; then
            # add props
            mkdir -p build/net45
            mkdir -p buildTransitive/net45
            cat ../../imageflow_x64.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
            cat ../../imageflow_x64.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH_2"
        elif [[ "${NUGET_RUNTIME}" == *'x86'* ]]; then
            # add props
            mkdir -p build/net45
            mkdir -p buildTransitive/net45
            cat ../../imageflow_x86.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
            cat ../../imageflow_x86.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH_2"
        elif [[ "${NUGET_RUNTIME}" == *'arm64'* ]]; then
            # add props
            mkdir -p build/net45
            mkdir -p buildTransitive/net45
            cat ../../imageflow_arm64.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH"
            cat ../../imageflow_arm64.targets | sed -e "s/:rid:/$NUGET_RUNTIME/g" > "$PROPS_PATH_2"
        fi
    fi
    
    if [[ "$1" == "tool" ]]; then
        NUSPEC_NAME="native_tool.nuspec"
    else
        NUSPEC_NAME="native.nuspec"
    fi
    
    cat ../../${NUSPEC_NAME} \
    | sed -e "s/:id:/${SED_NUGET_PACKAGE_NAME}/g" \
    | sed -e "s/:version:/${SED_NUGET_PACKAGE_VERSION}/g" \
    | sed -e "s/:repo_name_native:/${REPO_NAME}/g" \
    | sed -e "s/:repo_name_tool:/${REPO_NAME}/g" > "${NUGET_PACKAGE_NAME}.nuspec"
    
    
    echo "${NUGET_PACKAGE_NAME}.nuspec:"
    cat "${NUGET_PACKAGE_NAME}.nuspec"
    echo
    
    rm "${NUGET_OUTPUT_FILE}" || true
    zip -r "${NUGET_OUTPUT_FILE}" . || 7z a -tzip "${NUGET_OUTPUT_FILE}" "*" || powershell.exe -ExecutionPolicy Bypass -File "${SCRIPT_DIR}/zip.ps1" "${NUGET_OUTPUT_FILE}" "*"
    
    # verify file is not empty
    if [[ ! -s "${NUGET_OUTPUT_FILE}" ]]; then
        echo "Error: ${NUGET_OUTPUT_FILE} is empty"
        exit 1
    fi
    
    echo  "${NUGET_OUTPUT_FILE} packed"
    
)
