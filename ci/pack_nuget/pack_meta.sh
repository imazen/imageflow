#!/bin/bash
set -e # Exit on failure.
set -o pipefail 
# REQUIRES CI_TAG
# REQUIRES REPO_NAME
# REQUIRES REL_NUGET_OUTPUT_DIR

# --------------------------------------------------------------------------------
# Reasoning: Print the current working directory for debugging.
# --------------------------------------------------------------------------------
echo "Running pack_meta.sh from $(pwd)"

# --------------------------------------------------------------------------------
# Reasoning: Source the common utilities as in pack.sh.
# --------------------------------------------------------------------------------
source "$(dirname "${BASH_SOURCE[0]}")/utils.sh"

# --------------------------------------------------------------------------------
# Reasoning: Validate CI_TAG and extract the package version.
# --------------------------------------------------------------------------------
if [[ "$CI_TAG" == 'v'* ]]; then
    export NUGET_PACKAGE_VERSION="${CI_TAG#"v"}"
else
    echo "CI_TAG not set or invalid; skipping meta package creation"
    exit 0
fi

# --------------------------------------------------------------------------------
# Reasoning: Validate REL_NUGET_OUTPUT_DIR is set, ends with a slash, 
# and the directory exists.
# --------------------------------------------------------------------------------
if [[ -z "$REL_NUGET_OUTPUT_DIR" ]]; then
    echo "REL_NUGET_OUTPUT_DIR not set. Should be the location of the nuget output directory"
    exit 1
fi

if [[ "${REL_NUGET_OUTPUT_DIR: -1}" != "/" ]]; then
    echo "REL_NUGET_OUTPUT_DIR must end with a slash: $REL_NUGET_OUTPUT_DIR"
    exit 1
fi

if [[ ! -d "$REL_NUGET_OUTPUT_DIR" ]]; then
    echo "REL_NUGET_OUTPUT_DIR does not exist: $REL_NUGET_OUTPUT_DIR"
    exit 1
fi

# --------------------------------------------------------------------------------
# Reasoning: Validate REPO_NAME is set.
# --------------------------------------------------------------------------------
if [[ -z "$REPO_NAME" ]]; then
    echo "REPO_NAME not set. Should be the name of the repository"
    exit 1
fi

# --------------------------------------------------------------------------------
# Reasoning: Resolve paths similar to pack.sh.
# --------------------------------------------------------------------------------
SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$SCRIPT_DIR"; pwd)"
NUGET_OUTPUT_DIR="${SCRIPT_DIR}/../../${REL_NUGET_OUTPUT_DIR}"
mkdir -p "$NUGET_OUTPUT_DIR" || true

# --------------------------------------------------------------------------------
# Reasoning: Define the meta packages and their dependency runtimes.
# --------------------------------------------------------------------------------
declare -A PACKAGES=(
    ["Imageflow.NativeRuntime.All"]="win-x64 win-x86 win-arm64 linux-x64 linux-arm64 osx-x64 osx-arm64"
    ["Imageflow.NativeRuntime.All.x64"]="win-x64 linux-x64 osx-x64"
    ["Imageflow.NativeRuntime.All.Arm64"]="win-arm64 linux-arm64 osx-arm64"
    ["Imageflow.NativeRuntime.All.Windows"]="win-x64 win-x86 win-arm64"
    ["Imageflow.NativeRuntime.All.Linux"]="linux-x64 linux-arm64"
    ["Imageflow.NativeRuntime.All.Mac"]="osx-x64 osx-arm64"
    ["Imageflow.Net.All"]="Imageflow.NativeRuntime.All Imageflow.Net"
    ["Imageflow.Net.All.x64"]="Imageflow.NativeRuntime.All.x64 Imageflow.Net"
    ["Imageflow.Net.All.Arm64"]="Imageflow.NativeRuntime.All.Arm64 Imageflow.Net"
    ["Imageflow.Net.All.Windows"]="Imageflow.NativeRuntime.All.Windows Imageflow.Net"
    ["Imageflow.Net.All.Linux"]="Imageflow.NativeRuntime.All.Linux Imageflow.Net"
    ["Imageflow.Net.All.Mac"]="Imageflow.NativeRuntime.All.Mac Imageflow.Net"
)

# --------------------------------------------------------------------------------
# Reasoning: Function to generate an XML fragment for dependencies.
# --------------------------------------------------------------------------------
generate_dependencies() {
    local deps=($1)
    IMAGEFLOW_NET_VERSION=$(get_latest_version "Imageflow.Net")
    local xml=""
    xml+='    <dependencies>\n'
    xml+='      <group targetFramework=".NETStandard2.0">'
    for dep in "${deps[@]}"; do
        if [[ "$dep" == "Imageflow.Net" ]]; then
            # Use the latest version of Imageflow.Net, it's published separately   
            xml+="\n        <dependency id=\"$dep\" version=\"[${IMAGEFLOW_NET_VERSION}]\" />"
        elif [[ "$dep" =~ "Imageflow.NativeRuntime." ]]; then
            xml+="\n        <dependency id=\"$dep\" version=\"[${NUGET_PACKAGE_VERSION}]\" />"
        else
            xml+="\n        <dependency id=\"Imageflow.NativeRuntime.${dep}\" version=\"[${NUGET_PACKAGE_VERSION}]\" />"
        fi
    done
    xml+="\n      </group>\n"
    xml+="    </dependencies>"
    echo -e "$xml"
}

# --------------------------------------------------------------------------------
# Reasoning: Iterate over each meta package configuration and create package.
# --------------------------------------------------------------------------------
for PACKAGE_NAME in "${!PACKAGES[@]}"; do
    # Set the dependency string for the current package.
    DEPENDENCIES="${PACKAGES[$PACKAGE_NAME]}"
    export NUGET_COMBINED_NAME="$PACKAGE_NAME.$NUGET_PACKAGE_VERSION"
    echo "Generating package ${PACKAGE_NAME} with dependencies ${DEPENDENCIES}"
    
    # For meta packages, we use the package name directly.
    export NUGET_PACKAGE_NAME="${PACKAGE_NAME}"
    
    # --------------------------------------------------------------------------------
    # Reasoning: Create a random staging directory similar to pack.sh.
    # --------------------------------------------------------------------------------
    RANDOM_DIR_NAME=$(date +%s)_$RANDOM
    STAGING_DIR="${SCRIPT_DIR}/staging/${RANDOM_DIR_NAME}"
    mkdir -p "$STAGING_DIR" || true

    (
        # --------------------------------------------------------------------------------
        # Reasoning: Enter the staging directory, prepare a clean folder for the package,
        # and resolve the output package path.
        # --------------------------------------------------------------------------------
        cd "$STAGING_DIR"
        rm -rf "./$NUGET_COMBINED_NAME"
        mkdir "$NUGET_COMBINED_NAME"
        cd "$NUGET_COMBINED_NAME"
        
        # Define the NuGet package output file.
        NUGET_OUTPUT_FILE="${NUGET_OUTPUT_DIR}${NUGET_COMBINED_NAME}.nupkg"
        mkdir -p "${NUGET_OUTPUT_DIR}" || true
        
        # --------------------------------------------------------------------------------
        # Reasoning: Copy documentation files into the package.
        # --------------------------------------------------------------------------------
        cp "${SCRIPT_DIR}/README.md" .
        cp "${SCRIPT_DIR}/LICENSE.md" .
        
        # --------------------------------------------------------------------------------
        # Reasoning: Generate the dependencies XML using the helper function.
        # --------------------------------------------------------------------------------
        DEPS_XML=$(generate_dependencies "$DEPENDENCIES")
        
        # --------------------------------------------------------------------------------
        # Reasoning: Create a package description using the package name.
        # --------------------------------------------------------------------------------
        PACKAGE_DESCRIPTION="Imageflow is a fast, server-side-safe, and correct image processing library written in Rust. This package bundles ${PACKAGE_NAME#Imageflow.NativeRuntime.} native runtimes."
        
        # --------------------------------------------------------------------------------
        # Reasoning: Mirror pack.sh by escaping special characters and setting SED_REPO_NAME.
        # --------------------------------------------------------------------------------
        PACKAGE_DESCRIPTION="$(echo $PACKAGE_DESCRIPTION | sed -e 's/[\/&]/\\&/g')"
        DEPENDENCIES="$(echo $DEPENDENCIES | sed -e 's/[\/&]/\\&/g')"
        #if REPO_NAME contains \, fail
        if [[ "$REPO_NAME" == *"\\"* ]]; then
            echo "REPO_NAME contains a backslash: $REPO_NAME"
            exit 1
        fi
        SED_REPO_NAME="$(echo $REPO_NAME | sed -e 's/\//\\\//g')"

        # --------------------------------------------------------------------------------
        # Reasoning: Create the nuspec file using a multi-step sed process with error checks.
        # --------------------------------------------------------------------------------
        NUSPEC_NAME="${NUGET_PACKAGE_NAME}.nuspec"
        echo "Modifying template to create ${NUSPEC_NAME}, using values: ${NUGET_PACKAGE_NAME}, ${NUGET_PACKAGE_VERSION}, ${SED_REPO_NAME}, ${PACKAGE_DESCRIPTION}, ${DEPENDENCIES}"
        
        if [[ -z "$NUGET_PACKAGE_NAME" || -z "$NUGET_PACKAGE_VERSION" || -z "$SED_REPO_NAME" || -z "$PACKAGE_DESCRIPTION" || -z "$DEPENDENCIES" ]]; then
            echo "One or more variables are empty"
            exit 1
        fi

        # Step 1: Inject id and version.
        cat "${SCRIPT_DIR}/native_template.nuspec" \
        | sed -e "s/:id:/${NUGET_PACKAGE_NAME}/g" \
        | sed -e "s/:version:/${NUGET_PACKAGE_VERSION}/g" \
        > "${NUSPEC_NAME}.temp1" \
          || { echo "Failed to inject id and version (${NUGET_PACKAGE_NAME}, ${NUGET_PACKAGE_VERSION})"; exit 1; }

        # Step 2: Inject repo_name.
        cat "${NUSPEC_NAME}.temp1" | sed -e "s/:repo_name:/${SED_REPO_NAME}/g" > "${NUSPEC_NAME}.temp2" \
          || { echo "Failed to inject repo_name (${SED_REPO_NAME})"; exit 1; }
        
        # Step 3: Inject package_description.
        cat "${NUSPEC_NAME}.temp2" | sed -e "s/:package_description:/${PACKAGE_DESCRIPTION}/g" > "${NUSPEC_NAME}.temp3" \
          || { echo "Failed to inject package_description (${PACKAGE_DESCRIPTION})"; exit 1; }
        
        # Step 4: Inject dependencies.
        cat "${NUSPEC_NAME}.temp3" | sed -e "s/:dependencies:/${DEPENDENCIES}/g" > "${NUSPEC_NAME}.temp4" \
          || { echo "Failed to inject dependencies (${DEPENDENCIES})"; exit 1; }

        # Finalize the nuspec file.
        mv "${NUSPEC_NAME}.temp4" "${NUSPEC_NAME}"
        rm "${NUSPEC_NAME}.temp1" "${NUSPEC_NAME}.temp2" "${NUSPEC_NAME}.temp3"
        
        # --------------------------------------------------------------------------------
        # Reasoning: Output the resulting nuspec for debugging and validate it.
        # --------------------------------------------------------------------------------
        echo "${NUSPEC_NAME}:"
        cat "${NUSPEC_NAME}"
        echo "----------------------------------------"
        if [[ ! -f "${NUSPEC_NAME}" || ! -s "${NUSPEC_NAME}" ]]; then
            echo "Failed to create ${NUSPEC_NAME} using sed, empty or missing"
            exit 1
        fi  

        # --------------------------------------------------------------------------------
        # Reasoning: Copy package metadata files.
        # --------------------------------------------------------------------------------
        mkdir -p _rels
        cat "${SCRIPT_DIR}/.rels" | sed -e "s/:nuspec_name:/${NUGET_PACKAGE_NAME}.nuspec/g" > "_rels/.rels"
        cp "${SCRIPT_DIR}/[Content_Types].xml" .
        
        # --------------------------------------------------------------------------------
        # Reasoning: List all files included before packaging.
        # --------------------------------------------------------------------------------
        if [[ -f "${NUGET_OUTPUT_FILE}" ]]; then
            rm "${NUGET_OUTPUT_FILE}" || true
        fi
        
        echo "Packing ${NUGET_OUTPUT_FILE} with the following files:"
        echo "----------------------------------------"
        find . -type f -printf '%P\n' || find . -type f  #osx doesn't support -printf
        echo "----------------------------------------"
        
        # --------------------------------------------------------------------------------
        # Reasoning: Create the package by calling the shared create_package function.
        # --------------------------------------------------------------------------------
        echo "Attempting to create package..."
        if ! create_package "${NUGET_OUTPUT_FILE}" "$STAGING_DIR"; then
            echo "Failed to create package ${NUGET_PACKAGE_NAME}"
            exit 1
        fi
        
        echo "${NUGET_OUTPUT_FILE} packed successfully"
    )
done
