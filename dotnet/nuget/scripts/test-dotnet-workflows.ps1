# PowerShell script to test the consolidated build-pipeline.ps1 script locally

param(
    [string]$PackageVersion = "0.0.1-localtest" # Use a specific version for local testing
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# --- Helper Functions ---
# Helper function to get host RID (needed to determine which binaries to copy and pass to SingleTest mode)
function Get-HostRid {
    $os = $null
    $arch = $null
    if ($IsWindows) {
        $os = "win"
        $arch = switch ($env:PROCESSOR_ARCHITECTURE) {
            "AMD64" { "x64" }
            "ARM64" { "arm64" }
            "x86"   { "x86" }
            default { throw "Unsupported Windows Architecture for Get-HostRid: $($env:PROCESSOR_ARCHITECTURE)" }
        }
    } elseif ($IsMacOS) {
        $os = "osx"
        $unameArch = (uname -m).Trim()
        $arch = switch ($unameArch) {
            "x86_64" { "x64" }
            "arm64"  { "arm64" } # Apple Silicon reports arm64
            default { throw "Unsupported macOS Architecture for Get-HostRid: $unameArch" }
        }
    } elseif ($IsLinux) {
        $os = "linux"
        $unameArch = (uname -m).Trim()
        $arch = switch ($unameArch) {
            "x86_64" { "x64" }
            "aarch64" { "arm64" }
            # Add other Linux architectures if needed
            default { throw "Unsupported Linux Architecture for Get-HostRid: $unameArch" }
        }
    } else {
        throw "Unsupported OS for Get-HostRid"
    }

    return "$($os)-$($arch)"
}

# Helper function to get expected native binary name based on RID
function Get-NativeBinaryName {
    param([string]$rid)
    if ($rid.StartsWith("win-")) { return "imageflow.dll" }
    if ($rid.StartsWith("osx-")) { return "libimageflow.dylib" }
    if ($rid.StartsWith("linux-")) { return "libimageflow.so" }
    throw "Cannot determine native binary name for RID: $rid"
}

# Helper function to get expected native tool name based on RID
function Get-NativeToolName {
    param([string]$rid)
    if ($rid.StartsWith("win-")) { return "imageflow_tool.exe" }
    return "imageflow_tool" # Linux, macOS
}


# --- Script Setup ---

# Get script directory and workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
# Workspace root is three levels up from dotnet/nuget/scripts/
$WorkspaceRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..")

# Define paths
$BuildPipelineScript = Join-Path $ScriptDir "build-pipeline.ps1"

# This directory will hold the REAL artifacts copied from the Rust build for the HOST RID ONLY
$CurrentBuildArtifactDir = Join-Path $WorkspaceRoot "dotnet/nuget/artifacts/native/current_build"
$LocalPackOutputDir = Join-Path $WorkspaceRoot "temp_local_pack_output" # Where build-pipeline outputs packages

# Define Rust build output path (relative to WorkspaceRoot where cargo runs)
$RustBuildOutputDir = Join-Path $WorkspaceRoot "target/release"

# Check if dependent scripts exist
if (-not (Test-Path $BuildPipelineScript)) {
    Write-Error "Dependency script not found: $BuildPipelineScript"
    exit 1
}

# Ensure output directories are clean before starting
if (Test-Path $CurrentBuildArtifactDir) {
    Write-Host "Cleaning previous local build artifact directory: $CurrentBuildArtifactDir"
    Remove-Item -Recurse -Force $CurrentBuildArtifactDir
}
New-Item -ItemType Directory -Path $CurrentBuildArtifactDir | Out-Null

if (Test-Path $LocalPackOutputDir) {
    Write-Host "Cleaning previous local pack output directory: $LocalPackOutputDir"
    Remove-Item -Recurse -Force $LocalPackOutputDir
}
New-Item -ItemType Directory -Path $LocalPackOutputDir | Out-Null


Write-Host "--- Testing DotNet Build Pipeline Locally ---`n"

# --- Test Execution ---
try {
    # 0. Build Rust Projects
    Write-Host "`nStep 0: Building Rust projects (cargo build --release)..." -ForegroundColor Cyan
    Push-Location $WorkspaceRoot # Run cargo from workspace root
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo build returned non-zero exit code: $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
    Write-Host "✅ Rust build completed." -ForegroundColor Green

    # 1. Copy built native artifacts for HOST RID to CurrentBuildArtifactDir
    Write-Host "`nStep 1: Copying built native artifacts for HOST RID to $CurrentBuildArtifactDir..." -ForegroundColor Cyan
    $hostRid = Get-HostRid
    Write-Host "  Detected Host RID: $hostRid"

    # Determine expected artifact names for host RID
    $hostLibName = Get-NativeBinaryName -rid $hostRid
    $hostToolName = Get-NativeToolName -rid $hostRid
    $sourceLibPath = Join-Path $RustBuildOutputDir $hostLibName
    $sourceToolPath = Join-Path $RustBuildOutputDir $hostToolName

    if (-not (Test-Path $sourceLibPath)) { throw "Source library '$sourceLibPath' not found in Rust build output '$RustBuildOutputDir'" }
    if (-not (Test-Path $sourceToolPath)) { throw "Source tool '$sourceToolPath' not found in Rust build output '$RustBuildOutputDir'" }

    # Copy artifacts directly into the target dir
    Copy-Item -Path $sourceLibPath -Destination (Join-Path $CurrentBuildArtifactDir $hostLibName) -Force
    Copy-Item -Path $sourceToolPath -Destination (Join-Path $CurrentBuildArtifactDir $hostToolName) -Force
    Write-Host "✅ Host artifacts ($hostLibName, $hostToolName) copied." -ForegroundColor Green

    # 2. Run the consolidated build-pipeline.ps1 in SingleTest mode
    Write-Host "`nStep 2: Running build-pipeline.ps1 -Mode SingleTest ..." -ForegroundColor Cyan
    $pipelineArgs = @{
        Mode                 = 'SingleTest'
        TargetRid            = $hostRid
        PackageVersion       = $PackageVersion
        ImageflowNetVersion  = '*-*' # Use appropriate version constraint
        NativeArtifactBasePath = $CurrentBuildArtifactDir # Point to dir with only host artifacts
        PackOutputDirectory  = $LocalPackOutputDir
        Configuration        = 'Release' # Or 'Debug' if testing debug builds
        SkipTest             = $false # Run the test integrated in the pipeline
        # No push flags needed for SingleTest
    }
    & $BuildPipelineScript @pipelineArgs # Splat the arguments
    Write-Host "✅ Step 2 (build-pipeline.ps1) completed." -ForegroundColor Green


    Write-Host "`nLocal build pipeline test completed successfully." -ForegroundColor Green
    Write-Host "Generated packages are in: $LocalPackOutputDir" -ForegroundColor Cyan

} catch {
    # Catch errors from any script call
    Write-Error "❌ Workflow script test FAILED: $($_.Exception.Message)"
    if ($_.ErrorRecord) {
        $InvocationInfo = $_.ErrorRecord.InvocationInfo
        if ($InvocationInfo) {
             Write-Error "❌ Error occurred in script: $($InvocationInfo.ScriptName) at line $($InvocationInfo.ScriptLineNumber), Position $($InvocationInfo.OffsetInLine)"
             Write-Error "❌ Line: $($InvocationInfo.Line.Trim())"
        }
         Write-Error "❌ Full Error Record: $($_.ErrorRecord | Out-String)"
    } else {
         Write-Error "❌ Exception StackTrace: $($_.ScriptStackTrace)"
    }
    exit 1
} finally {
    # --- Cleanup ---
    Write-Host "`n--- Cleaning up temporary directories ---"
    if (Test-Path $CurrentBuildArtifactDir) {
        Write-Host "Removing local build artifact directory: $CurrentBuildArtifactDir"
        Remove-Item -Recurse -Force $CurrentBuildArtifactDir -ErrorAction SilentlyContinue
    }
    if (Test-Path $LocalPackOutputDir) {
        Write-Host "Removing local pack output directory: $LocalPackOutputDir"
        Remove-Item -Recurse -Force $LocalPackOutputDir -ErrorAction SilentlyContinue
    }
}

Write-Host "`nLocal workflow test script finished." -ForegroundColor Cyan
