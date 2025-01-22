#!/bin/bash

# ------------------------------------------------------------------------------
# Reasoning: This script checks the GLIBC version requirements of a given dynamic
# library and ensures they do not exceed a specified maximum. It also identifies
# other dynamic dependencies of the library.
#
# Goals:
# 1. Verify that the GLIBC version required by the dynamic library does not
#    exceed the maximum allowed version.
# 2. List all dynamic dependencies of the library for further inspection.
# ------------------------------------------------------------------------------

# Exit immediately if a command exits with a non-zero status
set -e

# ------------------------------------------------------------------------------
# Goal: Validate the number of input parameters
# ------------------------------------------------------------------------------
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <path_to_dynamic_library> <max_glibc_allowed>"
    exit 1
fi

# Assign input parameters to variables
DYNAMIC_LIB_PATH="$1"
MAX_GLIBC_ALLOWED="$2"

# ------------------------------------------------------------------------------
# Goal: Check if the dynamic library exists
# ------------------------------------------------------------------------------
if [ ! -f "$DYNAMIC_LIB_PATH" ]; then
    echo "Error: Dynamic library not found at path '$DYNAMIC_LIB_PATH'"
    exit 1
fi

# ------------------------------------------------------------------------------
# Goal: List dynamic dependencies using objdump and ldd
# ------------------------------------------------------------------------------
echo "Listing dynamic dependencies for $DYNAMIC_LIB_PATH:"

# Use ldd to list dynamic dependencies
ldd "$DYNAMIC_LIB_PATH" || {
    echo "Error: Failed to list dynamic dependencies using ldd."
    exit 1
}

# ------------------------------------------------------------------------------
# Goal: Extract and list GLIBC version requirements
# ------------------------------------------------------------------------------
echo "Extracting GLIBC version requirements:"

GLIBC_VERSIONS=$(objdump -T "$DYNAMIC_LIB_PATH" | grep GLIBC_ | sed 's/.*GLIBC_\([0-9.]*\).*/\1/g' | sort -Vu)

echo "GLIBC versions required by $DYNAMIC_LIB_PATH:"
echo "$GLIBC_VERSIONS"

# ------------------------------------------------------------------------------
# Goal: Determine the highest GLIBC version required
# ------------------------------------------------------------------------------
HIGHEST_GLIBC=$(echo "$GLIBC_VERSIONS" | sort -V | tail -n1)
echo "Highest GLIBC version required: $HIGHEST_GLIBC"

# ------------------------------------------------------------------------------
# Goal: Compare the highest GLIBC version with the maximum allowed version
# ------------------------------------------------------------------------------
if [ "$(printf '%s\n' "$MAX_GLIBC_ALLOWED" "$HIGHEST_GLIBC" | sort -V | tail -n1)" != "$MAX_GLIBC_ALLOWED" ]; then
    echo "Error: GLIBC version $HIGHEST_GLIBC exceeds the maximum allowed version $MAX_GLIBC_ALLOWED."
    exit 1
else
    echo "GLIBC version $HIGHEST_GLIBC is within the allowed limit of $MAX_GLIBC_ALLOWED."
fi

# ------------------------------------------------------------------------------
# Goal: Identify and list other real dynamic dependencies
# ------------------------------------------------------------------------------
echo "Identifying other dynamic dependencies:"

# Extract dependencies that are not GLIBC
OTHER_DEPENDENCIES=$(ldd "$DYNAMIC_LIB_PATH" | grep "=>" | awk '{print $3}' | grep -v 'glibc')

if [ -z "$OTHER_DEPENDENCIES" ]; then
    echo "No other dynamic dependencies found."
else
    echo "Other dynamic dependencies:"
    echo "$OTHER_DEPENDENCIES"
fi

# ------------------------------------------------------------------------------
# Goal: Exit successfully if all checks pass
# ------------------------------------------------------------------------------
echo "GLIBC and dynamic dependency checks passed successfully."
exit 0
