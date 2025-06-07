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

if ! command -v 7z >/dev/null 2>&1; then
    echo "Error: 7z command is required but not found in PATH." >&2
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
elif [[ "$absolute_output_path" == *.nupkg ]]; then
    archive_type="zip"
elif [[ "$absolute_output_path" == *.tar.gz ]]; then
    archive_type="tar.gz"
else
    echo "Error: Unsupported archive extension for '$absolute_output_path'. Use .zip or .tar.gz." >&2
    exit 1
fi

echo "Attempting to create archive '$absolute_output_path' (type: $archive_type) using 7z from base '$base_directory' including '$path_to_include'"

# Remove existing archive if it exists
rm -f "$absolute_output_path"

# --- Archiving Logic (using 7z exclusively) ---
(
    # Change into the base directory
    cd "$base_directory" || { echo "Error: Failed to cd into '$base_directory'." >&2; exit 1; }
    echo "Changed working directory to $(pwd)"

    # Ensure the path to include exists within the base directory
    if [ ! -e "$path_to_include" ]; then
        echo "Error: Path to include '$path_to_include' does not exist within '$base_directory' ('$(pwd)')." >&2
        exit 1
    fi

    # --- ZIP Creation ---
    if [[ "$archive_type" == "zip" ]]; then
        sevenz_exit_code=1
        if [[ "$path_to_include" == "." ]]; then
            echo "Archiving all content (including dotfiles) with 7z -tzip using recursive mode..."
            # Use 7z's recursive ability
            if find . -mindepth 1 -print -quit | grep -q .; then
                if 7z a -tzip -mx=9 "$absolute_output_path" . -r -x'!.DS_Store' > /dev/null; then
                    sevenz_exit_code=0
                else
                     sevenz_exit_code=$?
                     echo "7z -tzip (recursive mode) command failed with exit code $sevenz_exit_code." >&2
                fi
            else
                echo "Warning: No files found to archive with 7z -tzip. Creating empty archive."
                # Create an empty zip archive
                7z a -tzip "$absolute_output_path" -mx=0 > /dev/null # Add nothing, creates empty zip
                sevenz_exit_code=0 # Consider empty archive creation a success
            fi
        else
            echo "Archiving specific path with 7z -tzip: $path_to_include"
            if 7z a -tzip -mx=9 "$absolute_output_path" "$path_to_include" -x'!.DS_Store' > /dev/null; then
                 sevenz_exit_code=0
            else
                 sevenz_exit_code=$?
                 echo "7z -tzip (specific path) command failed with exit code $sevenz_exit_code." >&2
            fi
        fi
        if [ $sevenz_exit_code -eq 0 ]; then
            echo "Now unzipping the archive to a temp directory to check if it's valid..."
            temp_dir=$(mktemp -d)
            if 7z x -tzip "$absolute_output_path" -o"$temp_dir"; then
                echo "Archive is valid."
                sevenz_exit_code=0
            else
                echo "Archive failed to unzip."
                sevenz_exit_code=1
            fi
            rm -rf "$temp_dir"
        fi
                
        if [ $sevenz_exit_code -eq 0 ]; then
            echo "Archive created: '$absolute_output_path'. Now running diagnostics..."
            echo "---"
            echo "1. Which 7z executable was used?"
            which 7z
            echo "---"
            echo "2. What is the 7z version?"
            7z i | head -n 3
            echo "---"
            echo "3. What does the 'file' command think the archive is?"
            file "$absolute_output_path"
            echo "---"
            echo "4. Can 7z list the contents of the archive it just created?"
            if 7z l "$absolute_output_path" > /dev/null; then
                echo "SUCCESS: 7z was able to list the archive contents."
            else
                echo "FAILURE: 7z was NOT able to list the archive contents."
                sevenz_exit_code=1 # Mark as failure
            fi
            echo "---"
        fi
        exit $sevenz_exit_code

    fi

    # --- TAR.GZ Creation ---
    if [[ "$archive_type" == "tar.gz" ]]; then
        intermediate_tar="${absolute_output_path%.tar.gz}.intermediate.tar"
        rm -f "$intermediate_tar"
        sevenz_tar_exit_code=1
        sevenz_gzip_exit_code=1

        # Step 1: Create .tar
        if [[ "$path_to_include" == "." ]]; then
            echo "Creating intermediate tar (all content, incl dotfiles) with 7z -ttar using recursive mode..."
            if find . -mindepth 1 -print -quit | grep -q .; then
                if 7z a -ttar "$intermediate_tar" . -r -x'!.DS_Store' > /dev/null; then
                    sevenz_tar_exit_code=0
                else
                    sevenz_tar_exit_code=$?
                    echo "7z -ttar (recursive mode) command failed with exit code $sevenz_tar_exit_code." >&2
                fi
            else
                echo "Warning: No files found to archive with 7z -ttar. Creating empty archive."
                7z a -ttar "$intermediate_tar" > /dev/null # Creates empty tar
                sevenz_tar_exit_code=0
            fi
        else
            echo "Creating intermediate tar (specific path) with 7z -ttar: $path_to_include"
            if 7z a -ttar "$intermediate_tar" "$path_to_include" -x'!.DS_Store' > /dev/null; then
                 sevenz_tar_exit_code=0
            else
                 sevenz_tar_exit_code=$?
                 echo "7z -ttar (specific path) command failed with exit code $sevenz_tar_exit_code." >&2
            fi
        fi

        # Step 2: Compress .tar to .tar.gz if .tar creation succeeded
        if [ $sevenz_tar_exit_code -eq 0 ]; then
            echo "Compressing intermediate tar to gzip with 7z -tgzip..."
            if 7z a -tgzip "$absolute_output_path" "$intermediate_tar" > /dev/null; then
                sevenz_gzip_exit_code=0
            else
                sevenz_gzip_exit_code=$?
                echo "7z -tgzip command failed with exit code $sevenz_gzip_exit_code." >&2
            fi
        fi

        # Step 3: Clean up intermediate tar
        rm -f "$intermediate_tar"

        # Exit with success only if both steps succeeded
        if [ $sevenz_tar_exit_code -eq 0 ] && [ $sevenz_gzip_exit_code -eq 0 ]; then
            exit 0
        else
            exit 1
        fi
    fi # End tar.gz creation block

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
