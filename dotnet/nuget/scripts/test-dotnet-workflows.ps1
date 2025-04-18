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
# $PlaceholderScript = Join-Path $ScriptDir "create_native_placeholders.ps1" # No longer using placeholders
$SinglePackScript = Join-Path $ScriptDir "single-pack-and-test.ps1"
$MergePackScript = Join-Path $ScriptDir "merge-pack-test-publish.ps1"

# This directory will now hold the REAL artifacts copied from the Rust build
$TempStagingDir = Join-Path $WorkspaceRoot "temp_native_staging_for_test" 
$TempSinglePackOutputDir = Join-Path $WorkspaceRoot "temp_single_pack_output"
$FinalPackOutputDir = Join-Path $WorkspaceRoot "artifacts/nuget" # Where merge script outputs

# Define Rust build output path
$RustBuildOutputDir = Join-Path $WorkspaceRoot "target/release"

# Check if dependent scripts exist
# if (-not (Test-Path $PlaceholderScript)) { # Removed placeholder check
#     Write-Error "Dependency script not found: $PlaceholderScript"
#     exit 1
# }
if (-not (Test-Path $SinglePackScript)) {
    Write-Error "Dependency script not found: $SinglePackScript"
    exit 1
}
if (-not (Test-Path $MergePackScript)) {
    Write-Error "Dependency script not found: $MergePackScript"
    exit 1
}

# Ensure staging directory is clean before copying
if (Test-Path $TempStagingDir) {
    Write-Host "Cleaning previous staging directory: $TempStagingDir"
    Remove-Item -Recurse -Force $TempStagingDir
}
New-Item -ItemType Directory -Path $TempStagingDir | Out-Null

Write-Host "--- Testing DotNet Workflow Scripts Locally ---`n"

# --- Test Execution --- 
try {
    # 0. Build Rust Projects
    Write-Host "`nStep 0: Building Rust projects (cargo build --release)..." -ForegroundColor Cyan
    Push-Location $WorkspaceRoot # Run cargo from workspace root
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "❌ Cargo build FAILED."
        Pop-Location
        exit 1
    }
    Pop-Location
    Write-Host "✅ Rust build completed." -ForegroundColor Green

    # 1. Copy REAL native artifacts to staging directory for ALL RIDs (using win-x64 for local test)
    Write-Host "`nStep 1: Copying built native artifacts to $TempStagingDir for all expected RIDs..." -ForegroundColor Cyan
    
    # Define expected artifact names (adjust if needed)
    $NativeDllName = "imageflow.dll"
    $NativeToolName = "imageflow_tool.exe"
    $SourceDllPath = Join-Path $RustBuildOutputDir $NativeDllName
    $SourceToolPath = Join-Path $RustBuildOutputDir $NativeToolName

    # List of RIDs expected by the solution pack (add more if needed)
    $TargetRids = @(
        "win-x64", 
        "win-x86", 
        "win-arm64", 
        "linux-x64", 
        "linux-arm64", 
        "osx-x64", 
        "osx-arm64"
        # Add ubuntu-x86_64 if needed, maps to linux-x64 usually
    )

    foreach ($rid in $TargetRids) {
        # Determine expected names based on RID (simplified for copy)
        $TargetDllName = $NativeDllName
        $TargetToolName = $NativeToolName
        if ($rid -notlike "win-*") { # Non-windows
             $ext = if ($rid -like "osx-*") {"dylib"} else {"so"}
             $TargetDllName = "libimageflow.$ext"
             $TargetToolName = "imageflow_tool"
        }
        
        # Create target structure
        $TargetNativeDir = Join-Path $TempStagingDir $rid "native"
        New-Item -ItemType Directory -Path $TargetNativeDir -Force | Out-Null
        
        # Copy artifacts (using win-x64 source for all in this local test)
        Copy-Item -Path $SourceDllPath -Destination (Join-Path $TargetNativeDir $TargetDllName) -Force
        Copy-Item -Path $SourceToolPath -Destination (Join-Path $TargetNativeDir $TargetToolName) -Force
        Write-Host "  Copied win-x64 artifacts AS $TargetDllName/$TargetToolName for RID $rid."
    }
    Write-Host "✅ Artifacts copied for all RIDs." -ForegroundColor Green

    # 2. Run single-pack-and-test for host RID using REAL artifacts
    Write-Host "`nStep 2: Running single-pack-and-test.ps1 for host RID..." -ForegroundColor Cyan
    # Note: We don't need to explicitly pass RID, the script defaults to host
    & $SinglePackScript -PackageVersion $PackageVersion `
                       -ImageflowNetVersion *-* `
                       -NativeArtifactBasePath $TempStagingDir `
                       -PackOutputDirectory $TempSinglePackOutputDir
    Write-Host "✅ Step 2 completed." -ForegroundColor Green

    # --- DEBUG: Inspect the created native runtime package ---
    Write-Host "`n--------- DEBUG START: Inspect Native Package ---------" -ForegroundColor Magenta
    $NativePackagePath = Get-ChildItem -Path $TempSinglePackOutputDir -Filter "Imageflow.NativeRuntime.win-x86_64.*.nupkg" | Select-Object -First 1 -ExpandProperty FullName
    if ($NativePackagePath -and (Test-Path $NativePackagePath)) {
        $InspectDir = Join-Path $WorkspaceRoot "temp_inspect_nupkg"
        if (Test-Path $InspectDir) { Remove-Item -Recurse -Force $InspectDir }
        New-Item -ItemType Directory -Path $InspectDir | Out-Null
        Write-Host "Inspecting package: $NativePackagePath"
        Write-Host "Extracting to: $InspectDir ..."
        try {
            Expand-Archive -Path $NativePackagePath -DestinationPath $InspectDir -Force -ErrorAction Stop
            Write-Host "Extraction successful. Contents:"
            Get-ChildItem -Path $InspectDir -Recurse | ForEach-Object { Write-Host $_.FullName }
        } catch {
            Write-Warning "Failed to extract or list package contents: $($_.Exception.Message)"
        }
        # Keep the extracted dir for manual inspection if needed, cleanup happens in finally
    } else {
        Write-Warning "Could not find native runtime package in '$TempSinglePackOutputDir' matching 'Imageflow.NativeRuntime.win-x86_64.*.nupkg' to inspect."
    }
    Write-Host "--------- DEBUG END: Inspect Native Package -----------" -ForegroundColor Magenta
    # --- END DEBUG ---

    # 3. Run merge-pack-test-publish using REAL artifacts 
    #    (Simulates CI where artifacts are downloaded to CombinedNativeArtifactBasePath)
    Write-Host "`nStep 3: Running merge-pack-test-publish.ps1 (Pack and Test only)..." -ForegroundColor Cyan
    & $MergePackScript -PackageVersion $PackageVersion `
                      -ImageflowNetVersion *-* `
                      -CombinedNativeArtifactBasePath $TempStagingDir ` # Use the same staging dir
                      -PushToNuGet:$false `
                      -PushToGitHub:$false # Explicitly disable push
    Write-Host "✅ Step 3 completed." -ForegroundColor Green

    Write-Host "`nAll workflow script tests completed successfully." -ForegroundColor Green

} catch {
    # Catch errors from any script call
    Write-Error "❌ Workflow script test FAILED: $($_.Exception.Message)"
    if ($_.Exception.ErrorRecord -and $_.Exception.ErrorRecord.InvocationInfo) { # Check InvocationInfo
        Write-Error "❌ Error occurred in script: $($_.Exception.ErrorRecord.InvocationInfo.ScriptName) at line $($_.Exception.ErrorRecord.InvocationInfo.ScriptLineNumber)"
    }
    if ($_.Exception.ErrorRecord) {
        Write-Error "❌ Originating Error Record: $($_.Exception.ErrorRecord | Out-String)"
    }
    exit 1
} finally {
    # --- Cleanup --- 
    Write-Host "`n--- Cleaning up temporary directories ---"
    # Add cleanup for inspect dir
    $InspectDir = Join-Path $WorkspaceRoot "temp_inspect_nupkg"
    if (Test-Path $InspectDir) {
        Write-Host "Removing inspect directory: $InspectDir"
        Remove-Item -Recurse -Force $InspectDir -ErrorAction SilentlyContinue
    }
    # Existing cleanup
    if (Test-Path $TempStagingDir) {
        Write-Host "Removing staging directory: $TempStagingDir"
        Remove-Item -Recurse -Force $TempStagingDir -ErrorAction SilentlyContinue
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
