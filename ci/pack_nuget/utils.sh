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

# Function for creating NuGet package with cross-platform compatibility
create_package() {
    local output_file="$1"
    local staging_dir="$2"
    
    # Convert staging_dir to absolute path
    staging_dir="$(cd "$staging_dir" && pwd)"
    
    # If output_file is relative, make it relative to current directory, not staging_dir
    if [[ "${output_file:0:1}" != "/" ]]; then
        output_file="$(pwd)/$output_file"
    fi
    
    detect_platform
    
    # Ensure temp directory exists and is writable
    local temp_dir="${TMPDIR:-/tmp}"
    if [[ ! -d "$temp_dir" ]] || [[ ! -w "$temp_dir" ]]; then
        temp_dir="."
        echo "Warning: Using current directory for temporary files"
    fi
    # make random subdirectory in temp_dir
    local temp_subdir="${temp_dir}/$(date +%s)_$RANDOM"
    mkdir -p "$temp_subdir"
    
    # Create unique temp files
    local ps_log="${temp_subdir}/ps_$(date +%s)_$RANDOM.log"
    local ditto_log="${temp_subdir}/ditto_$(date +%s)_$RANDOM.log"
    local zip_log="${temp_subdir}/zip_$(date +%s)_$RANDOM.log"
    local sevenzip_log="${temp_subdir}/7z_$(date +%s)_$RANDOM.log"
    
    # Cleanup function for temp files
    cleanup_logs() {
        rm -f "$ps_log" "$ditto_log" "$zip_log" "$sevenzip_log"
    }
    trap cleanup_logs  1 2 3 6
    
    echo "Using temp directory: $temp_subdir"
    echo "Creating package from folder: $staging_dir"
    echo "Output file: $output_file"
    
    ( cd "$staging_dir"
        # Windows-specific handling
        if [[ "$PLATFORM" == "windows" ]]; then
            
            # replace /c/ with C:/
            WIN_SCRIPT_DIR=$(echo "$UTILS_SCRIPT_DIR" | sed 's/\/c\//C:\//g')
            echo "Using zip.ps1: $WIN_SCRIPT_DIR/zip.ps1"

            # Convert paths to Windows format for PowerShell
            
            # Convert paths to Windows format for PowerShell
            if ! powershell.exe -ExecutionPolicy Bypass -File "${WIN_SCRIPT_DIR}/zip.ps1" -ArchiveFile "${output_file}" -Paths . > "${ps_log}" 2>&1; then
                echo "PowerShell compression failed with output:"
                cat "${ps_log}"
                return 1
            fi
            return 0
        fi
        
        # macOS-specific handling
        if [[ "$PLATFORM" == "macos" ]] && command -v ditto >/dev/null 2>&1; then
            echo "Using macOS ditto..."
            if ! ditto -c -k --sequesterRsrc --keepParent . "${output_file}" > "${ditto_log}" 2>&1; then
                echo "ditto failed with output:"
                cat "${ditto_log}"
                return 1
            fi
            return 0
        fi
        
        # Try zip with different options
        if command -v zip >/dev/null 2>&1; then
            echo "Using zip..."
            if ! zip -r -q "${output_file}" . > "${zip_log}" 2>&1; then
                echo "Standard zip failed, trying without quiet flag..."
                if ! zip -r "${output_file}" . > "${zip_log}" 2>&1; then
                    echo "zip failed with output:"
                    cat "${zip_log}"
                else
                    return 0
                fi
            else
                return 0
            fi
        fi
        
        # Try 7z with different options
        if command -v 7z >/dev/null 2>&1; then
            echo "Using 7z..."
            if ! 7z a -tzip "${output_file}" . > "${sevenzip_log}" 2>&1; then
                echo "Standard 7z failed, trying with wildcard..."
                if ! 7z a -tzip "${output_file}" "*" > "${sevenzip_log}" 2>&1; then
                    echo "7z failed with output:"
                    cat "${sevenzip_log}"
                else
                    return 0
                fi
            else
                return 0
            fi
        fi
        
        echo "Error: No working compression method found"
        return 1
    )
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
    local temp_file="${temp_root}/nuget_versions_$(date +%s)_$RANDOM.json"
    
    # Cleanup function for temp file
    cleanup_temp() {
        rm -f "$temp_file"
    }
    trap cleanup_temp  1 2 3 6
    
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
