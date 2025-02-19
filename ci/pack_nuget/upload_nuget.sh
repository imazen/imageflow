#!/bin/bash
set -euo pipefail

# Usage: ./upload_nuget.sh <directory> <api_key>
# Returns: 0 if all packages uploaded successfully, 1 if any failed
# Outputs: List of failed packages and their error messages

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <directory> <api_key>"
    exit 1
fi

DIRECTORY="$1"
API_KEY="$2"
FAILED_PACKAGES=()
FAILED_MESSAGES=()

# Ensure directory exists and contains .nupkg files
if [ ! -d "$DIRECTORY" ]; then
    echo "Error: Directory '$DIRECTORY' does not exist"
    exit 1
fi

PACKAGES=$(find "$DIRECTORY" -name "*.nupkg")
if [ -z "$PACKAGES" ]; then
    echo "Error: No .nupkg files found in '$DIRECTORY'"
    exit 1
fi

# Process each package
while IFS= read -r package; do
    echo "Uploading $(basename "$package")..."
    
    # First get the response body and true exit code
    RESPONSE=$(curl -L "https://www.nuget.org/api/v2/package" \
        -H "X-NuGet-ApiKey: $API_KEY" \
        -H "X-NuGet-Client-Version: 4.1.0" \
        -A "NuGet Command Line/3.4.4.1321 (Unix 4.4.0.92)" \
        --upload-file "$package" \
        -v \
        2>&1)
    CURL_EXIT=$?
    
    # Get the status code from the response headers
    STATUS=$(echo "$RESPONSE" | grep -i "< HTTP/" | tail -n1 | awk '{print $3}')
    
    if [ $CURL_EXIT -ne 0 ] || [ -z "$STATUS" ] || [ "$STATUS" -ge 400 ]; then
        FAILED_PACKAGES+=("$package")
        FAILED_MESSAGES+=("$RESPONSE")
        echo "❌ Failed to upload $(basename "$package")"
        echo "   Status: ${STATUS:-CURL_ERROR}"
        echo "   Response:"
        echo "$RESPONSE"
    else
        echo "✅ Successfully uploaded $(basename "$package")"

        if [ "${DELETE_FROM_NUGET_AFTER_UPLOAD:-}" = "true" ]; then
            
            # To parse the package ID, look for the first [0-9]+ that has a dot before and after it. 
            #From that to the end is the version. Before that first dot is the package ID.
            packageId=$(basename "$package" | sed -n 's/\([0-9]\+\.\)[^.]*/\1/p')
            packageVersion=$(basename "$package" | sed -n 's/[0-9]\+\.\([0-9]\+\)\.nupkg/\1/p')
            echo "Deleting $packageId $packageVersion from nuget.org"
            dotnet nuget delete $packageId $packageVersion --source https://api.nuget.org/v3/index.json --non-interactive --api-key $API_KEY
        fi
    fi
done <<< "$PACKAGES"

# Report results
if [ ${#FAILED_PACKAGES[@]} -eq 0 ]; then
    echo "All packages uploaded successfully!"
    exit 0
else
    echo "Failed to upload ${#FAILED_PACKAGES[@]} package(s):"
    for i in "${!FAILED_PACKAGES[@]}"; do
        echo "Package: ${FAILED_PACKAGES[$i]}"
        echo "Error: ${FAILED_MESSAGES[$i]}"
        echo "---"
    done
    exit 1
fi 
