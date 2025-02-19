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
    
    # Try to upload the package and capture the response
    RESPONSE=$(curl -L "https://www.nuget.org/api/v2/package" \
        -H "X-NuGet-ApiKey: $API_KEY" \
        -H "X-NuGet-Client-Version: 4.1.0" \
        -A "NuGet Command Line/3.4.4.1321 (Unix 4.4.0.92)" \
        --upload-file "$package" \
        -w "\n%{http_code}" \
        --silent || echo "CURL_ERROR")

    # Split response into body and status code
    BODY=$(echo "$RESPONSE" | sed '$d')
    STATUS=$(echo "$RESPONSE" | tail -n1)

    if [ "$STATUS" = "CURL_ERROR" ] || [ "$STATUS" -ge 400 ]; then
        FAILED_PACKAGES+=("$package")
        FAILED_MESSAGES+=("$BODY")
        echo "❌ Failed to upload $(basename "$package")"
        echo "   Status: $STATUS"
        echo "   Response: $BODY"
    else
        echo "✅ Successfully uploaded $(basename "$package")"
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
