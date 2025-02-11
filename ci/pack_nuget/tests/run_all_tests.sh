#!/bin/bash
set -e

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Make all test scripts executable
chmod +x ./*.sh

# Function to run a test and report its status
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    echo "Running $test_name..."
    echo "----------------------------------------"
    
    if eval "$test_command"; then
        echo "✅ $test_name passed"
        return 0
    else
        echo "❌ $test_name failed"
        return 1
    fi
}

# Array to store failed tests
failed_tests=()

# Run all bash tests
for test_script in test_*.sh; do
    if [[ "$test_script" != "run_all_tests.sh" ]]; then
        if ! run_test "$test_script" "./$test_script"; then
            failed_tests+=("$test_script")
        fi
        echo
    fi
done

# Run PowerShell test if on Windows
if [[ "$OSTYPE" == "msys"* ]] || [[ "$OSTYPE" == "mingw"* ]]; then
    if ! run_test "test_zip.ps1" "powershell.exe -ExecutionPolicy Bypass -File ./test_zip.ps1"; then
        failed_tests+=("test_zip.ps1")
    fi
    echo
fi

# Report results
echo "========================================="
echo "Test Summary:"
echo "----------------------------------------"

if [ ${#failed_tests[@]} -eq 0 ]; then
    echo "✅ All tests passed!"
    exit 0
else
    echo "❌ The following tests failed:"
    for test in "${failed_tests[@]}"; do
        echo "   - $test"
    done
    exit 1
fi 
