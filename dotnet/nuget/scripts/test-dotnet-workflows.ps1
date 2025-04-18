# PowerShell script to test the single-pack-and-test and merge-pack-test-publish scripts locally

param(
    [string]$PackageVersion = "0.0.1-localtest" # Use a specific version for local testing
)

$ErrorActionPreference = "Stop"

# --- Script Setup ---

# Get script directory and workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
# Workspace root is three levels up from dotnet/nuget/scripts/
$WorkspaceRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..") 



# Define paths
# Scripts are now in the same directory as this script
$PlaceholderScript = Join-Path $ScriptDir "create_native_placeholders.ps1"
$SinglePackScript = Join-Path $ScriptDir "single-pack-and-test.ps1"
$MergePackScript = Join-Path $ScriptDir "merge-pack-test-publish.ps1"

$TempStagingDir = Join-Path $WorkspaceRoot "temp_placeholder_staging_for_test"
$TempSinglePackOutputDir = Join-Path $WorkspaceRoot "temp_single_pack_output"
$FinalPackOutputDir = Join-Path $WorkspaceRoot "artifacts/nuget" # Where merge script outputs

# Check if dependent scripts exist
if (-not (Test-Path $PlaceholderScript)) {
    Write-Error "Dependency script not found: $PlaceholderScript"
    exit 1
}
if (-not (Test-Path $SinglePackScript)) {
    Write-Error "Dependency script not found: $SinglePackScript"
    exit 1
}
if (-not (Test-Path $MergePackScript)) {
    Write-Error "Dependency script not found: $MergePackScript"
    exit 1
}

Write-Host "--- Testing DotNet Workflow Scripts Locally ---`n" -ForegroundColor Yellow

# --- Test Execution --- 
try {
    # 1. Create placeholder artifacts
    Write-Host "`nStep 1: Creating placeholder native artifacts..." -ForegroundColor Cyan
    & $PlaceholderScript -StagingDirectory $TempStagingDir

    # 2. Run single-pack-and-test for host RID using placeholders
    Write-Host "`nStep 2: Running single-pack-and-test.ps1 for host RID..." -ForegroundColor Cyan
    # Note: We don't need to explicitly pass RID, the script defaults to host
    & $SinglePackScript -PackageVersion $PackageVersion `
                       -ImageflowNetVersion *-* `
                       -NativeArtifactBasePath $TempStagingDir `
                       -PackOutputDirectory $TempSinglePackOutputDir
    Write-Host "✅ Step 2 completed." -ForegroundColor Green

    # 3. Run merge-pack-test-publish using placeholders (simulating combined artifacts)
    #    In a real CI scenario, CombinedNativeArtifactBasePath would point to downloaded artifacts.
    #    Here, we use the SAME placeholder directory for simplicity, assuming all needed files are there.
    Write-Host "`nStep 3: Running merge-pack-test-publish.ps1 (Pack and Test only)..." -ForegroundColor Cyan
    & $MergePackScript -PackageVersion $PackageVersion `
                      -ImageflowNetVersion *-* `
                      -CombinedNativeArtifactBasePath $TempStagingDir `
                      -PushToNuGet:$false `
                      -PushToGitHub:$false # Explicitly disable push
    Write-Host "✅ Step 3 completed." -ForegroundColor Green

    Write-Host "`nAll workflow script tests completed successfully." -ForegroundColor Green

} catch {
    # Catch errors from any script call
    Write-Error "❌ Workflow script test FAILED: $($_.Exception.Message)"
    if ($_.Exception.ErrorRecord) {
        Write-Error "❌ Originating Error Record: $($_.Exception.ErrorRecord | Out-String)"
    }
    exit 1
} finally {
    # --- Cleanup --- 
    Write-Host "`n--- Cleaning up temporary directories ---" -ForegroundColor Yellow
    if (Test-Path $TempStagingDir) {
        Write-Host "Removing placeholder directory: $TempStagingDir"
        Remove-Item -Recurse -Force $TempStagingDir
    }
    if (Test-Path $TempSinglePackOutputDir) {
        Write-Host "Removing single pack output directory: $TempSinglePackOutputDir"
        Remove-Item -Recurse -Force $TempSinglePackOutputDir
    }
     # Optionally clean the final pack output dir if desired after local test
    # if (Test-Path $FinalPackOutputDir) {
    #     Write-Host "Removing final pack output directory: $FinalPackOutputDir"
    #     Remove-Item -Recurse -Force $FinalPackOutputDir
    # }
}

Write-Host "`nLocal workflow test script finished." -ForegroundColor Cyan 
