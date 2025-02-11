<#
    Usage: powershell.exe -ExecutionPolicy Bypass -File .\test_zip.ps1
#>

# Get script directory and parent directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PackDir = Split-Path -Parent $ScriptDir

<#
    Reasoning: Step 1 - Set up our test environment.
    Goal: Create a temporary folder with test files and define test archive filenames.
#>
# Create a temporary directory for testing
$TestDir = Join-Path $env:TEMP "zip-test-$(Get-Random)"
New-Item -ItemType Directory -Path $TestDir | Out-Null

# Create sample files
$FileA = Join-Path $TestDir "fileA.txt"
$FileB = Join-Path $TestDir "fileB.log"
New-Item -ItemType File -Path $FileA | Out-Null
New-Item -ItemType File -Path $FileB | Out-Null

# Create a subdirectory, also containing a sample file
$SubDir = Join-Path $TestDir "subfolder"
New-Item -ItemType Directory -Path $SubDir | Out-Null
$FileC = Join-Path $SubDir "fileC.txt"
New-Item -ItemType File -Path $FileC | Out-Null

# Define archives to be created (with and without .zip extension)
$ArchiveForwardSlash = Join-Path $TestDir "archive_forward"
$ArchiveBackwardSlash = Join-Path $TestDir "archive_backward.zip"

<#
    Reasoning: Step 2 - Test using forward slashes in paths.
    Goal: Verify that zip.ps1 correctly handles forward slashes and appends .zip if missing.
#>
Write-Host "`n--- Testing zip.ps1 with forward slashes ---"

# Invoke zip.ps1 with forward slash paths and no .zip extension
Push-Location $PackDir
try {
    & "$PackDir\zip.ps1" $ArchiveForwardSlash $FileA.Replace('\','/') $FileB.Replace('\','/') 
}
catch {
    Write-Error "Test failed with forward slash paths: $_"
}
Pop-Location

Write-Host "Checking if archive was created (with .zip extension appended)..."
$ExpectedForwardSlashZip = $ArchiveForwardSlash + ".zip"
if (Test-Path "$ExpectedForwardSlashZip") {
    Write-Host "SUCCESS: Archive with forward slashes created as expected: $ExpectedForwardSlashZip"
} else {
    Write-Error "FAIL: Archive with forward slashes was not found."
}

<#
    Reasoning: Step 3 - Test using backward slashes in paths.
    Goal: Verify that zip.ps1 handles backward slashes properly and respects existing .zip in archive name.
#>
Write-Host "`n--- Testing zip.ps1 with backward slashes ---"

Push-Location $PackDir
try {
    & "$PackDir\zip.ps1" $ArchiveBackwardSlash "$FileA" "$FileB" "$SubDir"
}
catch {
    Write-Error "Test failed with backward slash paths: $_"
}
Pop-Location

Write-Host "Checking if archive was created (already included .zip extension)..."
if (Test-Path "$ArchiveBackwardSlash") {
    Write-Host "SUCCESS: Archive with backward slashes created as expected: $ArchiveBackwardSlash"
} else {
    Write-Error "FAIL: Archive with backward slashes was not found."
}

<#
    Reasoning: Step 4 - Teardown and clean up test artifacts.
    Goal: Remove test files and directories after testing is complete.
#>
Write-Host "`n--- Cleaning up test artifacts ---"
try {
    Remove-Item -Path $TestDir -Recurse -Force
    Write-Host "Cleanup successful. Test completed."
}
catch {
    Write-Host "Cleanup failed: $_"
} 
