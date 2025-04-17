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

# Function to verify NuGet package structure. 2nd and later parameters are strings that should be present in the .nuspec file.
verify_nupkg() {
    local nupkg_path="$1"
    local expected_strings=("${@:2}")
    
    # Validate input
    if [ ! -f "$nupkg_path" ]; then
        echo "Error: Package file not found: $nupkg_path" >&2
        return 1
    fi
    
    if [[ ! "$nupkg_path" =~ \.nupkg$ ]]; then
        echo "Error: File must have .nupkg extension: $nupkg_path" >&2
        return 1
    fi
    
    # Create temp directory with unique name
    local temp_dir="${TMPDIR:-/tmp}/nupkg-verify-$(date +%s)-${RANDOM}-${RANDOM}-$$"
    echo "Creating temp directory: $temp_dir"
    mkdir -p "$temp_dir"
    
    # Ensure cleanup happens even on failure
    cleanup() {
        local exit_code=$?
        echo "Cleaning up temp directory" >&2
        rm -rf "$temp_dir"
        return 0 # Explicit return
    }
    trap cleanup 1 2 3 6 ERR EXIT
    
    echo "Extracting package to verify structure..."

    local extraction_success=false
    
    # Try unzip first
    if command -v unzip >/dev/null 2>&1; then
        echo "Using unzip..."
        if unzip -q "$nupkg_path" -d "$temp_dir"; then
            extraction_success=true
        fi
    fi
    
    # Try 7z if available
    if command -v 7z >/dev/null 2>&1; then
        echo "Using 7z..."
        if 7z x "$nupkg_path" -o"$temp_dir" >/dev/null; then
            extraction_success=true
        fi
    fi

    if [ "$extraction_success" = true ]; then
        local nuspec_count
        nuspec_count=$(find "$temp_dir" -maxdepth 1 -name "*.nuspec" | wc -l)
        
        if [ "$nuspec_count" -eq 0 ]; then
            echo "Error: No .nuspec file found in package root" >&2
            echo "unzip -l $nupkg_path"
            unzip -l "$nupkg_path"
            echo "Contents of temp directory:"
            ls -R "$temp_dir"
            return 1
        elif [ "$nuspec_count" -gt 1 ]; then
            echo "Error: Multiple .nuspec files found in package root" >&2
            return 1
        fi
      
        echo "Found .nuspec file: $(find "$temp_dir" -maxdepth 1 -name "*.nuspec" -exec basename {} \;)"

        # Check if all expected strings are present in the .nuspec file
        local nuspec_file="$temp_dir/$(find "$temp_dir" -maxdepth 1 -name "*.nuspec" -exec basename {} \;)"
        for expected_string in "${expected_strings[@]}"; do
            if ! grep -q "$expected_string" "$nuspec_file"; then
                echo "Error: Expected string '$expected_string' not found in .nuspec file:" >&2
                cat "$nuspec_file" >&2
                return 1
            fi
        done

        return 0
    fi
    
    # Fallback to PowerShell on Windows
    if [[ "$PLATFORM" == "windows" ]]; then
        echo "Using PowerShell..."
        # Convert path to Windows format
        local win_nupkg_path
        win_nupkg_path=$(echo "$nupkg_path" | sed 's/\/c\//C:\//g')
        local win_script_dir
        win_script_dir=$(echo "$UTILS_SCRIPT_DIR" | sed 's/\/c\//C:\//g')
        
        if powershell.exe -ExecutionPolicy Bypass -File "${win_script_dir}/verify_nupkg.ps1" -NupkgPath "$win_nupkg_path"; then
            return 0
        fi
    fi


    
    echo "Error: No suitable extraction method found (unzip, 7z, or PowerShell required)" >&2
    return 1
}
