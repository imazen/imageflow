# PowerShell script to test the dotnet pack logic using placeholder artifacts

param(
    [string]$PackageVersion = "0.0.1-test"
    # ImageflowNetVersion is now fetched automatically
)

$ErrorActionPreference = "Stop"

# Get script directory and workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$WorkspaceRoot = Resolve-Path (Join-Path $ScriptDir "..")

# Source utility functions
. (Join-Path $ScriptDir "utils.ps1")

# Define paths for dependent scripts and temporary directory
$placeholderScript = Join-Path $ScriptDir "create_native_placeholders.ps1"
$packScript = Join-Path $ScriptDir "pack_test_push_all.ps1"
$tempStagingDir = Join-Path $WorkspaceRoot "temp_placeholder_staging_for_test"

# Check if dependent scripts exist
if (-not (Test-Path $placeholderScript)) {
    Write-Error "Dependency script not found: $placeholderScript"
    exit 1
}
if (-not (Test-Path $packScript)) {
    Write-Error "Dependency script not found: $packScript"
    exit 1
}


Write-Host "--- Testing Pack Logic with Placeholders ---`n" -ForegroundColor Yellow

try {
    # 1. Create placeholder artifacts
    & $placeholderScript -StagingDirectory $tempStagingDir

    # 2. Run the pack script in pack-only mode using the placeholders and fetched version
    Write-Host "`nRunning: $packScript (Pack only mode)"
    & $packScript -PackageVersion $PackageVersion `
                   -ImageflowNetVersion 0.13.2 `
                   -NativeArtifactBasePath $tempStagingDir `
                   -PushToNuGet:$false # Explicitly false
    
    Write-Host "Pack logic test completed successfully." -ForegroundColor Green

} catch {
    # Catch errors from either script call
    Write-Error "Pack logic test FAILED: $($_.Exception.Message)"
    # Add more details if available from the exception
    if ($_.Exception.ErrorRecord) {
        Write-Error "Originating Error Record: $($_.Exception.ErrorRecord | Out-String)"
    }
    exit 1
} finally {
    # Clean up placeholder directory
    if (Test-Path $tempStagingDir) {
        Write-Host "`nCleaning up placeholder directory: $tempStagingDir"
        Remove-Item -Recurse -Force $tempStagingDir
    }
}

Write-Host "`nPack logic test script finished." -ForegroundColor Cyan 
