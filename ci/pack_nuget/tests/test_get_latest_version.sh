#!/bin/bash

# Load utils
source ../utils.sh

echo "Checking available HTTP tools..."
if command -v curl >/dev/null 2>&1; then
    echo "✓ curl is available"
    curl --version | head -n 1
else
    echo "✗ curl is not available"
fi

if command -v wget >/dev/null 2>&1; then
    echo "✓ wget is available"
    wget --version | head -n 1
else
    echo "✗ wget is not available"
fi

if command -v powershell.exe >/dev/null 2>&1; then
    echo "✓ powershell.exe is available"
else
    echo "✗ powershell.exe is not available"
fi

# Test connectivity to NuGet API
echo -e "\nTesting connectivity to NuGet API..."
API_URL="https://api.nuget.org/v3/index.json"

if command -v curl >/dev/null 2>&1; then
    echo "Testing with curl..."
    if curl -sSL --head "$API_URL" >/dev/null; then
        echo "✓ NuGet API is accessible via curl"
    else
        echo "✗ Cannot access NuGet API via curl"
    fi
fi

# Function to test version lookup
test_package_version() {
    local package_name="$1"
    echo -e "\nTesting version lookup with ${package_name}..."
    
    # Capture version (stdout) and debug info (stderr) separately
    if VERSION=$(get_latest_version "$package_name" 2>/dev/null); then
        # Show debug info in a controlled way
        echo "Debug output:"
        get_latest_version "$package_name" >/dev/null
        
        # Verify version format
        if [[ ! -z "$VERSION" ]] && [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+.*$ ]]; then
            echo "✅ Successfully retrieved version: $VERSION"
            return 0
        else
            echo "❌ Invalid version format: '$VERSION'"
            return 1
        fi
    else
        # Show debug info on failure
        echo "Debug output (failure):"
        get_latest_version "$package_name" >/dev/null
        echo "❌ Version lookup failed"
        return 1
    fi
}

# Test with known packages
test_package_version "Newtonsoft.Json" || exit 1
test_package_version "Imageflow.Net" || exit 1 
exit 0
