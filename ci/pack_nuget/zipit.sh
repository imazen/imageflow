#!/bin/bash
# Usage: zipit.sh <output_archive_path> <base_directory> <path_to_include>
#   output_archive_path: Absolute or relative path for the final archive file (.zip or .tar.gz).
#   base_directory: The directory to change into before archiving. Paths inside the archive will be relative to this.
#   path_to_include: The specific file or directory (or '.') within base_directory to add to the archive.

set -e
set -o pipefail

# Ensure utils.sh is available for detect_platform
UTILS_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${UTILS_SCRIPT_DIR}/utils.sh" # For detect_platform

# --- Input Validation ---
if [ "$#" -ne 3 ]; then
    echo "Usage: zipit.sh <output_archive_path> <base_directory> <path_to_include>" >&2
    exit 1
fi

output_archive_path="$1"
base_directory="$2"
path_to_include="$3"
original_dir=$(pwd)
absolute_output_path=""
archive_created=false

# Resolve absolute path for the output archive
if [[ "$output_archive_path" == /* ]]; then
    absolute_output_path="$output_archive_path"
else
    absolute_output_path="${original_dir}/${output_archive_path}" # Make absolute relative to original_dir
fi

# Ensure base directory exists
if [ ! -d "$base_directory" ]; then
    echo "Error: Base directory '$base_directory' does not exist." >&2
    exit 1
fi

# Determine archive type
archive_type=""
if [[ "$absolute_output_path" == *.zip ]]; then
    archive_type="zip"
elif [[ "$absolute_output_path" == *.tar.gz ]]; then
    archive_type="tar.gz"
else
    echo "Error: Unsupported archive extension for '$absolute_output_path'. Use .zip or .tar.gz." >&2
    exit 1
fi

echo "Attempting to create archive '$absolute_output_path' (type: $archive_type) from base '$base_directory' including '$path_to_include'"

# --- Archiving Logic ---
(
    # Change into the base directory
    cd "$base_directory" || { echo "Error: Failed to cd into '$base_directory'." >&2; exit 1; }
    echo "Changed working directory to $(pwd)"

    # Ensure the path to include exists within the base directory
    if [ ! -e "$path_to_include" ]; then
        echo "Error: Path to include '$path_to_include' does not exist within '$base_directory'." >&2
        exit 1
    fi

    # --- TAR.GZ Creation ---
    if [[ "$archive_type" == "tar.gz" ]]; then
        echo "Attempting tar..."
        tar_opts="--exclude=.DS_Store --ignore-failed-read --no-ignore-command-error"
        files_to_tar=""

        if [[ "$path_to_include" == "." ]]; then
            # Using '* .[!.]*' should grab normal files and dotfiles except . and ..
            echo "Archiving all content (including dotfiles)..."
            # Note: We capture the list of files first to handle the case where no dotfiles exist, which would cause '.[!.]*' to fail.
            files_to_tar=$(ls -d .[!.]* * 2>/dev/null || true)
        else
            # Archiving a specific path
            echo "Archiving specific path: $path_to_include"
            files_to_tar="$path_to_include"
        fi

        if [ -z "$files_to_tar" ]; then
            echo "Warning: No files found to tar in $(pwd) for pattern/path '$path_to_include'" >&2
            # Create empty tarball as tar would
            tar $tar_opts -czf "$absolute_output_path" --files-from /dev/null
        elif tar $tar_opts -czf "$absolute_output_path" $files_to_tar; then
            echo "tar succeeded."
        else
            echo "tar failed." >&2
            exit 1 # If tar fails, there's no standard fallback for .tar.gz
        fi
        exit 0 # Success (or handled empty case)
    fi

    # --- ZIP Creation (with fallbacks) ---
    if [[ "$archive_type" == "zip" ]]; then

        # Priority 1: PowerShell on Windows-like environments (including Git Bash/WSL from PS)
        if command -v powershell.exe >/dev/null 2>&1; then
            ps_script_path="${UTILS_SCRIPT_DIR}/zip.ps1"
            if [ -f "$ps_script_path" ]; then
                # Check for wslpath before attempting conversion
                if command -v wslpath >/dev/null 2>&1; then
                    echo "Attempting PowerShell zip.ps1 script (Priority 1)..."
                    win_script_path="$(wslpath -w "$ps_script_path")"
                    win_output_path="$(wslpath -w "$absolute_output_path")"
                    if [[ "$path_to_include" == "." ]]; then
                         win_include_path="."
                    else
                         # Need the absolute path of the include path for wslpath
                         abs_include_path="$(pwd)/$path_to_include" # pwd is base_directory here
                         win_include_path="$(wslpath -w "$abs_include_path")"
                    fi

                    echo "Running: powershell.exe -ExecutionPolicy Bypass -NoProfile -File \"${win_script_path}\" -ArchiveFile \"${win_output_path}\" -Paths \"${win_include_path}\""
                    if powershell.exe -ExecutionPolicy Bypass -NoProfile -File "${win_script_path}" -ArchiveFile "${win_output_path}" -Paths "${win_include_path}"; then
                        echo "PowerShell zip.ps1 succeeded."
                        # Verify file exists after success, PS script might not error correctly
                        if [ -f "$absolute_output_path" ]; then
                           exit 0 # Success
                        else
                           echo "PowerShell script reported success, but output file missing. Trying other methods..." >&2
                        fi
                    else
                        echo "PowerShell zip.ps1 failed. Trying other methods..." >&2
                    fi
                else
                     echo "wslpath command not found. Skipping PowerShell fallback." >&2
                fi # end wslpath check
            else
                echo "PowerShell script '$ps_script_path' not found. Trying other methods..." >&2
            fi
        fi

        # Fallback 1: Standard zip command
        if command -v zip >/dev/null 2>&1; then
            echo "Attempting zip command (Fallback 1)..."
            if zip -r -q --exclude ".DS_Store" "$absolute_output_path" "$path_to_include"; then
                echo "zip command succeeded."
                exit 0 # Success
            else
                echo "zip command failed." >&2
            fi
        else
             echo "zip command not found."
        fi

        # Fallback 2: 7z command
        if command -v 7z >/dev/null 2>&1; then
            echo "Attempting 7z command (Fallback 2)..."
            content_to_add_7z="$path_to_include"
            if [[ "$content_to_add_7z" == "." ]]; then
                content_to_add_7z="*"
            fi
            if 7z a -tzip -mx=9 "$absolute_output_path" $content_to_add_7z -x'!.DS_Store' > /dev/null; then
                echo "7z command succeeded."
                exit 0 # Success
            else
                echo "7z command failed." >&2
            fi
        else
            echo "7z command not found."
        fi

        # Fallback 3: macOS ditto (Only if PowerShell wasn't tried/failed AND platform is macOS)
        detect_platform # Sets $PLATFORM
        if [[ "$PLATFORM" == "macos" ]] && command -v ditto >/dev/null 2>&1; then
             # Check if powershell was available - if so, it was tried and failed, so don't try ditto as redundant?
             # Or maybe ditto works better? Let's keep it as a final fallback on macOS.
            echo "Attempting macOS ditto command (Fallback 3)..."
            if ditto -c -k --sequesterRsrc "$path_to_include" "$absolute_output_path"; then
                echo "ditto command succeeded."
                exit 0 # Success
            else
                echo "ditto command failed." >&2
            fi
        fi

        # If we reach here, all methods failed for zip
        echo "Error: Failed to create zip archive '$absolute_output_path' using any available method." >&2
        exit 1
    fi # End zip creation block

) # End subshell for cd

# Check subshell exit code
subshell_exit_code=$?
if [ $subshell_exit_code -eq 0 ]; then
    if [ -f "$absolute_output_path" ]; then
        echo "Successfully created archive '$absolute_output_path'"
        archive_created=true
    else
        echo "Error: Archiving command reported success, but output file '$absolute_output_path' not found." >&2
        exit 1
    fi
else
    echo "Error: Archiving failed with exit code $subshell_exit_code." >&2
    # Clean up potentially partially created archive
    rm -f "$absolute_output_path"
    exit $subshell_exit_code
fi

# Final check
if [ "$archive_created" = true ]; then
    exit 0
else
    # This case should theoretically not be reached due to checks above
    echo "Error: Archive creation failed unexpectedly." >&2
    exit 1
fi 
