#!/bin/bash
set -e

# Create a temporary directory for our build context
temp_dir=$(mktemp -d)
trap 'rm -rf "${temp_dir}"' EXIT

# Create a minimal static binary in the temp directory
cat > "${temp_dir}/imageflow_tool.c" << 'EOF'
#include <stdio.h>
int main() {
    printf("Hello from test binary\n");
    return 0;
}
EOF

# Compile as static binary
gcc -static "${temp_dir}/imageflow_tool.c" -o "${temp_dir}/imageflow_tool"

# Debug: Print locations
echo "Build context directory: ${temp_dir}"
echo "Dockerfile location: $(pwd)/Dockerfile"
ls -la "${temp_dir}"

# Build the Docker image from the temp directory
docker build -t imageflow_tool_test \
  -f "$(pwd)/Dockerfile" \
  "${temp_dir}"

echo "running the image"
# Test that the image runs
docker run --rm imageflow_tool_test

# Cleanup is handled by the trap 
