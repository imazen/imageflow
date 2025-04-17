#!/bin/bash
set -e # Exit on failure.
set -o pipefail # Ensure pipeline failures are caught.

# Usage: ./pack_meta_dotnet.sh
# Requires Environment Variables:
# - CI_TAG: The Git tag (e.g., v1.2.3) used to derive NUGET_PACKAGE_VERSION
# - REPO_NAME: The GitHub repository name (e.g., imazen/imageflow)
# - REL_NUGET_OUTPUT_DIR: Relative path to the directory where the .nupkg should be saved
# - REL_NUGET_ARCHIVE_DIR (Optional): Relative path to an archive directory

echo "Running pack_meta_dotnet.sh from $(pwd)"

# --------------------------------------------------------------------------------
# Source Utilities & Validate Inputs
# --------------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/utils.sh" # For verify_nupkg and get_latest_version

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

validate_env_var "CI_TAG"
validate_env_var "REPO_NAME"
validate_env_var "REL_NUGET_OUTPUT_DIR" "" "slash" # Don't check existence yet
validate_env_var "NUGET_PACKAGE_VERSION"

# --------------------------------------------------------------------------------
# Extract Version & Define Packages
# --------------------------------------------------------------------------------
# NUGET_PACKAGE_VERSION is now expected to be set by the calling environment (workflow/test script)
echo "Using NUGET_PACKAGE_VERSION: ${NUGET_PACKAGE_VERSION}"

# Define meta packages and their dependency runtimes/packages (Bash 3.2 compatible arrays)
PACKAGES_KEYS=(
    "Imageflow.NativeRuntime.All"
    "Imageflow.NativeRuntime.All.x64"
    "Imageflow.NativeRuntime.All.Arm64"
    "Imageflow.NativeRuntime.All.Windows"
    "Imageflow.NativeRuntime.All.Linux"
    "Imageflow.NativeRuntime.All.Mac"
    "Imageflow.Net.All"
    "Imageflow.Net.All.x64"
    "Imageflow.Net.All.Arm64"
    "Imageflow.Net.All.Windows"
    "Imageflow.Net.All.Linux"
    "Imageflow.Net.All.Mac"
)

PACKAGES_VALUES=(
    "win-x64 win-x86 win-arm64 linux-x64 linux-arm64 osx-x64 osx-arm64" # Native Runtime RIDs
    "win-x64 linux-x64 osx-x64" # Native Runtime RIDs
    "win-arm64 linux-arm64 osx-arm64" # Native Runtime RIDs
    "win-x64 win-x86 win-arm64" # Native Runtime RIDs
    "linux-x64 linux-arm64" # Native Runtime RIDs
    "osx-x64 osx-arm64" # Native Runtime RIDs
    "Imageflow.NativeRuntime.All Imageflow.Net" # Package IDs
    "Imageflow.NativeRuntime.All.x64 Imageflow.Net" # Package IDs
    "Imageflow.NativeRuntime.All.Arm64 Imageflow.Net" # Package IDs
    "Imageflow.NativeRuntime.All.Windows Imageflow.Net" # Package IDs
    "Imageflow.NativeRuntime.All.Linux Imageflow.Net" # Package IDs
    "Imageflow.NativeRuntime.All.Mac Imageflow.Net" # Package IDs
)

IMAGEFLOW_NET_VERSION=""
get_imageflow_net_version() {
    if [[ -z "$IMAGEFLOW_NET_VERSION" ]]; then
        echo "Fetching latest Imageflow.Net version..."
        IMAGEFLOW_NET_VERSION=$(get_latest_version "Imageflow.Net")
        if [[ -z "$IMAGEFLOW_NET_VERSION" ]]; then
            echo "Error: Failed to fetch latest version for Imageflow.Net"
            exit 1
        fi
        echo "Using Imageflow.Net version: $IMAGEFLOW_NET_VERSION"
    fi
}

# --------------------------------------------------------------------------------
# Helper function to generate PackageReference ItemGroup XML
# --------------------------------------------------------------------------------
generate_dependencies_xml() {
    local meta_package_name="$1" # e.g., Imageflow.NativeRuntime.All.x64 or Imageflow.Net.All
    local deps_string="$2"
    local deps_array=($deps_string)
    local xml=""
    xml+="  <ItemGroup>\n"

    local is_net_meta=false
    if [[ "$meta_package_name" == *".Net."* ]]; then
        is_net_meta=true
    fi

    for dep in "${deps_array[@]}"; do
        local dep_id=""
        local dep_version=""

        if [[ "$dep" == "Imageflow.Net" ]]; then
            if [[ "$is_net_meta" != true ]]; then
                echo "Error: Imageflow.Net dependency is only allowed in .Net.* meta packages, not in ${meta_package_name}"
                exit 1
            fi
            get_imageflow_net_version # Ensure version is fetched
            dep_id="Imageflow.Net"
            dep_version="$IMAGEFLOW_NET_VERSION"
        elif [[ "$dep" =~ ^Imageflow\.NativeRuntime\. ]]; then
            # Dependency is already a full Package ID (e.g., Imageflow.NativeRuntime.All)
            if [[ "$is_net_meta" != true ]]; then
                echo "Error: Direct dependency on ${dep} is only allowed in .Net.* meta packages, not in ${meta_package_name}"
                exit 1
            fi
            # Prevent meta packages depending on other meta packages other than direct children
            if [[ "$dep" != "Imageflow.NativeRuntime.All" &&
                  "$dep" != "Imageflow.NativeRuntime.All.x64" &&
                  "$dep" != "Imageflow.NativeRuntime.All.Arm64" &&
                  "$dep" != "Imageflow.NativeRuntime.All.Windows" &&
                  "$dep" != "Imageflow.NativeRuntime.All.Linux" &&
                  "$dep" != "Imageflow.NativeRuntime.All.Mac" ]]; then
                 echo "Error: Meta package ${meta_package_name} cannot depend on complex meta package ${dep}"
                 exit 1
            fi

            dep_id="$dep"
            dep_version="$NUGET_PACKAGE_VERSION"
        else
            # Dependency is an RID suffix, construct full NativeRuntime ID
            if [[ "$is_net_meta" == true ]]; then
                echo "Error: RID-based dependency '${dep}' is not allowed in .Net.* meta packages (${meta_package_name}), use full Package IDs instead."
                exit 1
            fi
            dep_id="Imageflow.NativeRuntime.${dep}"
            dep_version="$NUGET_PACKAGE_VERSION"
        fi

        xml+="    <PackageReference Include=\"${dep_id}\" Version=\"${dep_version}\" />\n"
    done

    xml+="  </ItemGroup>"
    echo -e "$xml"
}

# --------------------------------------------------------------------------------
# Resolve Paths
# --------------------------------------------------------------------------------
REPO_ROOT=$(resolve_path "${SCRIPT_DIR}/../..")
NUGET_OUTPUT_DIR="${REPO_ROOT}/${REL_NUGET_OUTPUT_DIR}"
NUGET_ARCHIVE_DIR=""
if [[ -n "$REL_NUGET_ARCHIVE_DIR" ]]; then
    NUGET_ARCHIVE_DIR="${REPO_ROOT}/${REL_NUGET_ARCHIVE_DIR}"
fi
META_CSPROJ_TEMPLATE="${SCRIPT_DIR}/templates/meta.csproj.template"

if [[ ! -f "$META_CSPROJ_TEMPLATE" ]]; then
    echo "Error: Meta project template not found: $META_CSPROJ_TEMPLATE"
    exit 1
fi

# --------------------------------------------------------------------------------
# Iterate and Pack Meta Packages
# --------------------------------------------------------------------------------
for (( i=0; i < ${#PACKAGES_KEYS[@]}; i++ )); do
    PACKAGE_NAME="${PACKAGES_KEYS[$i]}"
    DEPENDENCIES_STR="${PACKAGES_VALUES[$i]}"
    export NUGET_COMBINED_NAME="${PACKAGE_NAME}.${NUGET_PACKAGE_VERSION}"

    # Combine echo statements
    echo -e "===================================================\nGenerating meta package ${PACKAGE_NAME} (v${NUGET_PACKAGE_VERSION})\nDependencies string: ${DEPENDENCIES_STR}\n==================================================="

    # Use helper function to create staging dir and setup trap
    # Run main logic in a subshell to isolate trap
    (
        STAGING_DIR=$(create_staging_dir "${SCRIPT_DIR}/staging" "meta_${PACKAGE_NAME}") || exit 1
        TEMP_PACKAGE_DIR="${STAGING_DIR}/package_output"
        mkdir -p "$TEMP_PACKAGE_DIR"

        # Ensure cleanup is handled by trap from create_staging_dir
        # trap 'rm -rf "${STAGING_DIR}"' EXIT ERR INT TERM # REMOVED

        # Enter staging directory
        cd "$STAGING_DIR" || exit 1

        # 1. Copy template and common files
        if [[ ! -f "$META_CSPROJ_TEMPLATE" ]]; then # Check just before use
             echo "Error: Meta project template not found: $META_CSPROJ_TEMPLATE"
             exit 1
        fi
        cp "$META_CSPROJ_TEMPLATE" ./project.csproj
        cp "${SCRIPT_DIR}/README.md" .
        cp "${SCRIPT_DIR}/LICENSE.md" .
        cp "${SCRIPT_DIR}/icon.png" .
        echo "Copied template, README, LICENSE, icon"

        # 2. Generate dependencies XML
        DEPS_XML=$(generate_dependencies_xml "$PACKAGE_NAME" "$DEPENDENCIES_STR")
        echo "Generated Dependencies XML:"
        echo -e "$DEPS_XML"

        # 3. Inject dependencies XML into project.csproj
        # Use awk for reliable multi-line insertion, checking that insertion occurred.
        if ! awk -v xml="$DEPS_XML" '
        BEGIN { inserted=0 }
        # Match the line containing the marker text, ignoring comment syntax
        /PACKAGING SCRIPT MUST INSERT DEPENDENCIES HERE/ {
            print xml
            inserted=1
            # Skip the original marker line and the closing comment line if needed
            # This assumes the marker is unique and the XML block replaces it.
            # If the marker line itself should be kept, remove the 'next'
            next
        }
        { print }
        END { if (inserted == 0) { print "Error: Marker text 'PACKAGING SCRIPT MUST INSERT DEPENDENCIES HERE' not found in project.csproj template!" > "/dev/stderr"; exit 1 } }
        ' project.csproj > project.csproj.tmp; then
            echo "Error: awk command failed during dependency injection."
            exit 1
        fi
        mv project.csproj.tmp project.csproj

        echo "Injected dependencies into project.csproj"

        # Verify injection using grep
        if ! grep -q '<ItemGroup>' project.csproj; then
            echo "Error: Verification failed. Expected dependencies ItemGroup not found in project.csproj after injection."
            echo "--- project.csproj content after failed injection ---"
            cat project.csproj || echo "Failed to cat project.csproj"
            echo "--- End of project.csproj content ---"
            exit 1
        else
            echo "Verification passed: Found <ItemGroup> in project.csproj."
        fi

        echo "--- project.csproj --- START ---"
        cat project.csproj || echo "Failed to cat project.csproj"
        echo "--- project.csproj --- END ---"

        # 4. Determine Package Description
        if [[ "$PACKAGE_NAME" == *".Net."* ]]; then
            PACKAGE_DESCRIPTION="Meta-package for Imageflow.Net that bundles the required native runtime dependencies ($PACKAGE_NAME)."
        else
            PACKAGE_DESCRIPTION="Meta-package that bundles multiple Imageflow native runtime dependencies ($PACKAGE_NAME)."
        fi

        # Ensure Imageflow.Net version is available for the pack command
        get_imageflow_net_version

        # 5. Build dotnet pack command arguments
        PACK_ARGS=()
        # --output, --no-build, --no-restore handled by run_dotnet_pack
        PACK_ARGS+=("--configuration" "Release")
        PACK_ARGS+=("/p:PackageId=${PACKAGE_NAME}")
        PACK_ARGS+=("/p:Version=${NUGET_PACKAGE_VERSION}")
        PACK_ARGS+=("/p:PackageDescription=\"${PACKAGE_DESCRIPTION}\"") # Quotes needed
        PACK_ARGS+=("/p:RepositoryUrl=https://github.com/${REPO_NAME}")
        PACK_ARGS+=("/p:ImageflowNetVersion=${IMAGEFLOW_NET_VERSION}")
        PACK_ARGS+=("/p:NoWarn=NU5128")

        echo "----------------------------------------"
        echo "Calculated pack command arguments:"
        printf '%s\n' "${PACK_ARGS[@]}"
        echo "----------------------------------------"

        # 6. Execute dotnet pack using helper function
        FINAL_NUPKG_PATH=$(run_dotnet_pack "${STAGING_DIR}/project.csproj" "${PACKAGE_NAME}" "${NUGET_PACKAGE_VERSION}" "${TEMP_PACKAGE_DIR}" "${NUGET_OUTPUT_DIR}" "${NUGET_ARCHIVE_DIR}" "${PACK_ARGS[@]}") || exit 1

        # 7. Verify package using helper function
        echo "Verifying package contents using helper script..."
        # For meta packages, we also don't compare against a gold nuspec currently
        run_verify_script "${FINAL_NUPKG_PATH}" "" || exit 1

    ) # Exit staging subshell

    exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        echo "Error: Failed to pack meta package ${PACKAGE_NAME} in subshell."
        exit $exit_code
    fi

done

echo "All meta packages created and verified successfully." 
