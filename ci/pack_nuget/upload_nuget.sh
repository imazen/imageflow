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
FAILED_LOG_VAR=""

parse_version() {
    # To parse the package ID, look for the first [0-9]+ that has a dot before and after it.
    # From that to the end is the version. Before that first dot is the package ID.
    local package="$1"
    # remove the .nupkg suffix
    local name=$(basename "$package" .nupkg)
    local version='none'
    local re='\.([0-9]+\..+)'
    if [[ "$name" =~ $re ]]; then
        version=${BASH_REMATCH[1]}
        if ! [[ "$version" =~ ^([0-9]+\.)+([0-9]+-[a-zA-Z0-9]+)?$ ]]; then
            echo "❌ parse_version Invalid version: $version" > /dev/stderr
            exit 1
        fi
    else
        echo "❌ parse_version could not parse version from: $name, re: $re" > /dev/stderr
        exit 1
    fi
    local id="${name:0:${#name}-${#version}-1}"
    # now validate the version
    echo "$id $version"
}


# test parse_version
test_file="Imageflow.NativeRuntime.linux-x64.1.2.3-rc02.nupkg"
parse_result=$(parse_version "$test_file")
# assert that parse_result is "Imageflow.NativeRuntime 1.2.3-rc02"
if [ "$parse_result" != "Imageflow.NativeRuntime.linux-x64 1.2.3-rc02" ]; then
    echo "❌ parse_version failed for $test_file, expected: Imageflow.NativeRuntime.linux-x64 1.2.3-rc02, got: $parse_result"
    exit 1
fi

delete_package() {
    local full_path="$1"
    local packageId, packageVersion=$(parse_version "$full_path")
    echo "Deleting $packageId $packageVersion from nuget.org..."
    # copy any error output to stderr
    dotnet nuget delete $packageId $packageVersion --source https://api.nuget.org/v3/index.json --non-interactive --api-key $API_KEY 2>&1
    return $?
}

delete_all_packages_in_directory() {
    # we want to report if anything fails at the end, but also keep deleting packages   
    # we have set -euo pipefail set
    local directory="$1"
    local failed_list=()
    local failed_count=0
    for package in "$directory"/*.nupkg; do
        result=$(delete_package "$package")
        if [ $? -ne 0 ]; then
            failed_list+=("$package")
            failed_count=$((failed_count + 1))
            echo "❌ Failed to delete from nuget: $package" > /dev/stderr
        fi
    done
    return $failed_count
}

upload_package_curl() {
    local package="$1"
    local api_key="$2"
    local is_github=$3
    local name=$(basename "$package" .nupkg)
    if [ "$is_github" = "true" ]; then
        echo "Uploading \(with curl\) $name to github..."

        RESPONSE=$(curl -vX PUT -u "${{github.repository_owner}}:${{ secrets.GITHUB_TOKEN }}" -F package=@$f https://nuget.pkg.github.com/${{github.repository_owner}}/ 2>&1)
        CURL_EXIT=$?
    else
        echo "Uploading \(with curl\) $name to nuget.org..."
        # Try to upload the package and capture the response
        RESPONSE=$(curl -L "https://www.nuget.org/api/v2/package" \
            -H "X-NuGet-ApiKey: $API_KEY" \
        -H "X-NuGet-Client-Version: 4.1.0" \
        -A "NuGet Command Line/3.4.4.1321 (Unix 4.4.0.92)" \
        --upload-file "$package" \
            -w "\n%{http_code}" \
            --silent 2>&1)
        CURL_EXIT=$?
    fi

    # Split response into body and status code
    BODY=$(echo "$RESPONSE" | sed '$d')
    STATUS=$(echo "$RESPONSE" | tail -n1)

    if [ $CURL_EXIT -ne 0 ] || [ "$STATUS" -ge 400 ]; then
        FAILED_MESSAGES+=("$BODY\n$STATUS")
        echo "❌ Failed to upload $name with curl" > /dev/stderr
        echo "   Status: $STATUS" > /dev/stderr
        echo "   Response: $BODY" > /dev/stderr
        echo "   Response: $RESPONSE" > /dev/stderr
        return 1 
    else
        echo "✅ Successfully uploaded $name with curl"
        return 0
    fi
}

upload_package_dotnet() {
    local package="$1"
    local api_key="$2"
    local is_github=$3
    if [ "$is_github" = "true" ]; then
        echo "Uploading (with dotnet) $(basename "$package") to github..."
        output=$(dotnet nuget push "$package" --source https://nuget.pkg.github.com/${{github.repository_owner}}/ --api-key $API_KEY 2>&1)
    else
        echo "Uploading (with dotnet) $(basename "$package") to nuget.org..."
        output=$(dotnet nuget push "$package" --source https://api.nuget.org/v3/index.json --api-key $API_KEY 2>&1)
    fi
    if [ $? -ne 0 ]; then
        echo "❌ Failed to upload $(basename "$package") with dotnet "
        echo "$output" > /dev/stderr
        FAILED_MESSAGES+=("dotnet push failed: $output")
        return 1
    else
        echo "✅ Successfully uploaded $(basename "$package") with dotnet"
        echo "$output" > /dev/stdout
        return 0
    fi
}


echo "Searching for packages in $DIRECTORY"
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
    echo "Uploading $package" > /dev/stderr
    set +e
    RESULT_OUTPUT=$(upload_package_dotnet "$package" "$API_KEY" "${PUSH_TO_GITHUB:-false}" 2>&1)
    if [ $? -ne 0 ]; then
        FAILED_LOG_VAR="$FAILED_LOG_VAR\n$RESULT_OUTPUT"
        echo "$RESULT_OUTPUT" > /dev/stderr
        RESULT_OUTPUT_2=$(upload_package_curl "$package" "$API_KEY" "${PUSH_TO_GITHUB:-false}" 2>&1)
        if [ $? -ne 0 ]; then
            FAILED_LOG_VAR="$FAILED_LOG_VAR\n$RESULT_OUTPUT_2"
            echo "$RESULT_OUTPUT_2" > /dev/stderr
            FAILED_LOG_VAR="$FAILED_LOG_VAR\n❌ Failed to upload $package (no attemps remaining)"
            echo "❌ Failed to upload $package (no attemps remaining)" > /dev/stderr
            FAILED_PACKAGES+=("$package")
            FAILED_MESSAGES+=("❌ Failed to upload $package (tried dotnet and curl)\n$RESULT_OUTPUT\n$RESULT_OUTPUT_2")
        else
            echo "$RESULT_OUTPUT_2"
        fi
    else
        echo "$RESULT_OUTPUT"
    fi
    
done <<< "$PACKAGES"

eport results
should_delete=false
final_exit_code=0
if [ ${#FAILED_PACKAGES[@]} -eq 0 ]; then
    echo "✅ All packages uploaded successfully!"
    if [ "${DELETE_ALL_FROM_NUGET_AFTER_UPLOAD:-}" = "true" ]; then
        echo "Deleting all packages from nuget.org, because DELETE_ALL_FROM_NUGET_AFTER_UPLOAD=true"
        should_delete=true
    fi
    final_exit_code=0
else
    echo "❌ Failed to upload ${#FAILED_PACKAGES[@]} package(s):"
    for i in "${!FAILED_PACKAGES[@]}"; do
        echo "Package: ${FAILED_PACKAGES[$i]}"
        echo "Error: ${FAILED_MESSAGES[$i]}"
        echo "---"
    done
    echo "$FAILED_LOG_VAR"
    final_exit_code=1
fi 

if [ "$should_delete" = "true" ]; then
    if [ $(delete_all_packages_in_directory "$DIRECTORY") -ne 0 ]; then
        echo "❌ Failed to delete all packages from server"
        exit 2
    else
        echo "✅ Successfully deleted all packages from server"
    fi
fi
set -e
exit $final_exit_code
