#!/bin/bash

# Exit on error
set -e
set -o pipefail 

UTILS_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Platform detection function
detect_platform() {
    echo "Detecting platform..."
    echo "OSTYPE: $OSTYPE"
    echo "uname -s: $(uname -s)"
    echo "uname -m: $(uname -m)"
    
    case "$OSTYPE" in
        "darwin"*)
            echo "Detected macOS"
            PLATFORM="macos"
            ;;
        "msys"*|"mingw"*)
            echo "Detected Windows/Git Bash"
            PLATFORM="windows"
            ;;
        "linux"*)
            echo "Detected Linux"
            PLATFORM="linux"
            ;;
        *)
            echo "Warning: Unknown platform ($OSTYPE)"
            PLATFORM="unknown"
            ;;
    esac
    
    # Detect ARM64
    if [[ "$(uname -m)" == "arm64" ]] || [[ "$(uname -m)" == "aarch64" ]]; then
        echo "Detected ARM64 architecture"
        IS_ARM64=true
    else
        IS_ARM64=false
    fi
}

# Function for creating NuGet package with cross-platform compatibility using 7z
create_package() {
    local output_file="$1"
    local staging_dir="$2"
    
    # Convert staging_dir to absolute path
    staging_dir="$(cd "$staging_dir" && pwd)"
    
    # If output_file is relative, make it relative to current directory, not staging_dir
    if [[ "${output_file:0:1}" != "/" ]]; then
        output_file="$(pwd)/$output_file"
    fi
    
    echo "Creating package (zip) using 7z from folder: $staging_dir"
    echo "Output file: $output_file"
    
    # Ensure 7z is available
    if ! command -v 7z >/dev/null 2>&1; then
        echo "Error: 7z command is required by create_package but not found." >&2
        return 1
    fi
    
    # Remove existing archive if it exists
    rm -f "$output_file"
    
    (
        cd "$staging_dir" || exit 1
        # Use find piped to xargs to handle all files including dotfiles
        echo "Archiving all content with 7z -tzip using find..."
        # Check if find returns any files first
        if find . -mindepth 1 -print -quit | grep -q .; then
             # Use -mx=5 for reasonable compression, NuGet doesn't need max
             if ! (find . -mindepth 1 -print0 | xargs -0 7z a -tzip -mx=5 "$output_file" -x'!.DS_Store' > /dev/null); then
                echo "Error: 7z -tzip command failed for create_package."
                exit 1
             fi
        else
             echo "Warning: No files found to archive with 7z -tzip for create_package. Creating empty archive."
             if ! 7z a -tzip "$output_file" -mx=0 > /dev/null; then
                 echo "Error: Failed to create empty archive with 7z."
                 exit 1
             fi
        fi
        exit 0
    )
    local subshell_exit_code=$?
    
    if [ $subshell_exit_code -ne 0 ]; then
        echo "Error: Failed to create package $output_file in subshell."
        rm -f "$output_file" # Clean up potentially broken archive
        return 1
    fi
    
    if [ ! -f "$output_file" ]; then
         echo "Error: 7z reported success, but output file $output_file not found."
         return 1
    fi
    
    echo "Package $output_file created successfully."
    return 0
}

# Function to get last version from a JSON response
get_last_version() {
    local input_file="$1"
    # Match versions that start with a number, take the last one
    grep -o '"[0-9][^"]*"' "$input_file" | tr -d '"' | tail -n 1
}

# Function to get latest version of a NuGet package with improved reliability
get_latest_version() {
    local package_id="$1"
    # lowercase the package id
    package_id=$(echo "$package_id" | tr '[:upper:]' '[:lower:]')
    local api_url="https://api.nuget.org/v3-flatcontainer/${package_id}/index.json"
    
    # Send platform detection output to stderr
    detect_platform >&2
    
    # Ensure temp directory exists and is writable
    local temp_root="${TMPDIR:-/tmp}"
    if [[ ! -d "$temp_root" ]] || [[ ! -w "$temp_root" ]]; then
        temp_root="."
        echo "Warning: Using current directory for temporary files" >&2
    fi
    
    # Create unique temp file
    local temp_file="${temp_root}/nuget_versions_$(date +%s)_${RANDOM}.json"
    
    # Cleanup function for temp file
    cleanup_temp() {
        rm -f "$temp_file"
        return 0 # Explicit return
    }
    trap cleanup_temp  1 2 3 6 ERR EXIT
    
    local version=""
    
    echo "Fetching latest version for $package_id..." >&2
    echo "Using temp file: $temp_file" >&2
    
    # Try curl with multiple attempts
    if command -v curl >/dev/null 2>&1; then
        echo "Attempting to use curl..." >&2
        for i in {1..3}; do
            echo "Curl attempt $i..." >&2
            if curl -sSL --retry 3 --retry-delay 2 "$api_url" -o "$temp_file"; then
                version=$(get_last_version "$temp_file")
                if [ ! -z "$version" ]; then
                    echo "Successfully retrieved version: $version" >&2
                    echo "$version"
                    return 0
                fi
            fi
            sleep 2
        done
    fi
    
    # Fallback to wget with multiple attempts
    if command -v wget >/dev/null 2>&1; then
        echo "Attempting to use wget..." >&2
        for i in {1..3}; do
            echo "Wget attempt $i..." >&2
            if wget -q --tries=3 --timeout=15 -O "$temp_file" "$api_url"; then
                version=$(get_last_version "$temp_file")
                if [ ! -z "$version" ]; then
                    echo "Successfully retrieved version: $version" >&2
                    echo "$version"
                    return 0
                fi
            fi
            sleep 2
        done
    fi
    
    # Final fallback to PowerShell
    if [[ "$PLATFORM" == "windows" ]]; then
        echo "Attempting to use PowerShell..." >&2
        for i in {1..3}; do
            echo "PowerShell attempt $i..." >&2
            if powershell.exe -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (Invoke-WebRequest -Uri '$api_url' -UseBasicParsing).Content" > "$temp_file"; then
                version=$(get_last_version "$temp_file")
                if [ ! -z "$version" ]; then
                    echo "Successfully retrieved version: $version" >&2
                    echo "$version"
                    return 0
                fi
            fi
            sleep 2
        done
    fi
    
    echo "Error: Could not fetch latest version for $package_id" >&2
    return 1
}

# Use realpath for robustness if available, otherwise use cd/pwd fallback
resolve_path() {
    if command -v realpath > /dev/null; then
        realpath "$1"
    else
        # Ensure the directory exists before trying to cd into it
        if [[ -d "$1" ]]; then
            (cd "$1" && pwd)
        elif [[ -f "$1" ]]; then
            # Handle file path: get dir, cd, then append filename
            local dir
            dir=$(dirname "$1")
            local base
            base=$(basename "$1")
            if [[ -d "$dir" ]]; then
                echo "$(cd "$dir" && pwd)/$base"
            else
                 echo "Error: Cannot resolve directory for path '$1'" >&2
                 return 1
            fi
        else
            echo "Error: Path '$1' not found for resolve_path" >&2
            return 1
        fi
    fi
}

# Function to verify NuGet package structure. (REMOVED - Use verify_nupkg.ps1 instead)
# verify_nupkg() { ... removed ... }

# Function to safely create a staging directory and set up cleanup trap
# Usage: STAGING_DIR=$(create_staging_dir "${SCRIPT_DIR}/staging" "some_prefix")
# Returns the absolute path to the created directory
create_staging_dir() {
    local base_dir="$1"
    local prefix="$2"
    local random_suffix
    random_suffix=$(date +%s)_$RANDOM
    local staging_path
    staging_path="${base_dir}/${prefix}_${random_suffix}"

    # Redirect diagnostic messages to stderr
    echo "Creating staging directory: ${staging_path}" >&2
    mkdir -p "${staging_path}" || {
        echo "Error: Failed to create staging directory '${staging_path}'" >&2
        return 1
    }

    # Resolve to absolute path
    local absolute_staging_path
    absolute_staging_path=$(resolve_path "${staging_path}") || return 1

    # Redirect diagnostic message to stderr
    trap "echo 'Cleaning up staging directory: ${absolute_staging_path}' >&2; rm -rf '${absolute_staging_path}'" EXIT ERR INT TERM

    # Only echo the final path to stdout
    echo "${absolute_staging_path}"
    return 0
}

# Common function to execute dotnet pack, find the output, and move/copy it
# Usage: run_dotnet_pack "$staging_dir/project.csproj" "$NUGET_PACKAGE_NAME" "$NUGET_PACKAGE_VERSION" "$TEMP_PACKAGE_DIR" "$FINAL_OUTPUT_DIR" "$ARCHIVE_DIR_OR_EMPTY" "${PACK_ARGS[@]}"
run_dotnet_pack() {
    # Remove debug printing
    # set -x

    local csproj_path="$1"
    local package_name="$2"
    local package_version="$3" # Used for finding output, not passed to pack
    local temp_output_dir="$4"
    local final_output_dir="$5"
    local archive_dir="$6" # Can be empty
    shift 6 # Remove first 6 args, remaining are pack args
    local pack_args=("$@")

    echo "----------------------------------------"
    echo "Executing dotnet restore --no-dependencies..."
    # Add verbosity and a fixed RID for debugging
    if ! dotnet restore "${csproj_path}" --no-dependencies -r linux-x64 -v detailed; then
        echo "Error: dotnet restore --no-dependencies failed for ${csproj_path}."
        return 1
    fi

    echo "----------------------------------------"
    echo "Executing dotnet pack..."
    echo "Command: dotnet pack \"${csproj_path}\" --no-build --no-restore ${pack_args[*]}"
    if ! dotnet pack "${csproj_path}" --no-build --no-restore "${pack_args[@]}"; then
        echo "Error: dotnet pack command failed for ${package_name}."
        return 1
    fi
    echo "----------------------------------------"

    # Find the created package file
    # Search pattern allows for normalized versions (e.g., 1.0.0 instead of 1.0)
    local created_nupkg_path
    created_nupkg_path=$(find "${temp_output_dir}" -maxdepth 1 -name "${package_name}.${package_version}*.nupkg" -print -quit || find "${temp_output_dir}" -maxdepth 1 -name "${package_name}.*.nupkg" -print -quit || true)

    if [[ -z "$created_nupkg_path" || ! -f "$created_nupkg_path" ]]; then
        echo "Error: dotnet pack seemed to succeed, but failed to find a package matching '${package_name}.${package_version}*.nupkg' or '${package_name}.*.nupkg' in ${temp_output_dir}"
        echo "Listing contents of staging output dir (${temp_output_dir}):"
        ls -la "${temp_output_dir}"
        return 1
    fi

    local nupkg_filename
    nupkg_filename=$(basename "$created_nupkg_path")
    echo "Package created successfully: ${nupkg_filename}"

    mkdir -p "${final_output_dir}" || {
        echo "Error: Failed to create final output directory '${final_output_dir}'" >&2
        return 1
    }
    local final_nupkg_path="${final_output_dir}${nupkg_filename}"
    echo "Moving package to final destination: ${final_nupkg_path}"
    mv "$created_nupkg_path" "$final_nupkg_path" || {
        echo "Error: Failed to move '${created_nupkg_path}' to '${final_nupkg_path}'" >&2
        return 1
    }

    # Optional: Copy to archive directory
    if [[ -n "$archive_dir" ]]; then
        echo "Copying package to archive directory: ${archive_dir}"
        mkdir -p "${archive_dir}" || {
             echo "Error: Failed to create archive directory '${archive_dir}'" >&2
             return 1
        }
        cp "$final_nupkg_path" "${archive_dir}${nupkg_filename}" || {
            echo "Error: Failed to copy '${final_nupkg_path}' to '${archive_dir}${nupkg_filename}'" >&2
            return 1
        }
    fi

    # Return the final path for verification
    echo "${final_nupkg_path}"
    
    # Remove debug printing
    # set +x
    return 0
}

# Common function to call verify_nupkg.ps1
# Usage: run_verify_script "$final_nupkg_path" "$gold_nuspec_path_or_empty"
run_verify_script() {
    local nupkg_to_verify="$1"
    local gold_nuspec_path="$2" # Optional

    local verify_script_native="${UTILS_SCRIPT_DIR}/verify_nupkg.ps1"

    if [[ ! -f "$verify_script_native" ]]; then
        echo "Error: Verification script not found at ${verify_script_native}" >&2
        return 1
    fi
    if [[ -n "$gold_nuspec_path" && ! -f "$gold_nuspec_path" ]]; then
        echo "Error: Gold nuspec file specified but not found at ${gold_nuspec_path}" >&2
        return 1
    fi

    # Convert paths to Windows format for powershell.exe if wslpath exists
    local verify_script_win="$verify_script_native"
    local nupkg_path_win="$nupkg_to_verify"
    local gold_nuspec_path_win="$gold_nuspec_path"

    if command -v wslpath &> /dev/null; then
        verify_script_win=$(wslpath -w "$verify_script_native")
        nupkg_path_win=$(wslpath -w "$nupkg_to_verify")
        if [[ -n "$gold_nuspec_path" ]]; then
             gold_nuspec_path_win=$(wslpath -w "$gold_nuspec_path")
        fi
        echo "Converted paths for PowerShell: Script='$verify_script_win', Package='$nupkg_path_win', Gold='$gold_nuspec_path_win'" >&2
    else
        echo "Warning: wslpath command not found. Assuming native Windows or compatible paths." >&2
    fi

    local powershell_args=("-ExecutionPolicy" "Bypass" "-File" "${verify_script_win}" "-NupkgPath" "${nupkg_path_win}")
    if [[ -n "$gold_nuspec_path_win" ]]; then
        powershell_args+=("-GoldNuspecPath" "${gold_nuspec_path_win}")
    fi

    echo "Executing: powershell.exe ${powershell_args[*]}" >&2
    if ! powershell.exe "${powershell_args[@]}"; then
        echo "Error: Package verification failed for ${nupkg_to_verify}"
        return 1
    fi

    echo "Package verification succeeded for ${nupkg_to_verify}" >&2
    return 0
}
