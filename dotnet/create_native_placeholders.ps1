# PowerShell script to create a directory structure with empty placeholder native artifact files for all RIDs

param(
    [Parameter(Mandatory=$true)]
    [string]$StagingDirectory
)

$ErrorActionPreference = "Stop"

# Get script directory (optional, may not be needed if $StagingDirectory is absolute)
# $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

Write-Host "--- Creating Placeholder Native Artifacts ---`n" -ForegroundColor Yellow
Write-Host "Target Staging Directory: $StagingDirectory"

$rids = @(
    "win-x64",
    "win-x86",
    "win-arm64",
    "osx-x64",
    "osx-arm64",
    "linux-x64",
    "linux-arm64"
)

function Get-NativeBinaryNames($rid) {
    $libName = ""
    $toolName = ""
    if ($rid -like "win*") {
        $libName = "imageflow.dll"
        $toolName = "imageflow_tool.exe"
    } elseif ($rid -like "osx*") {
        $libName = "libimageflow.dylib"
        $toolName = "imageflow_tool"
    } else { # linux
        $libName = "libimageflow.so"
        $toolName = "imageflow_tool"
    }
    return @{ Lib = $libName; Tool = $toolName }
}

try {
    # Ensure base staging directory exists
    if (-not (Test-Path $StagingDirectory)) {
        Write-Host "Creating base directory: $StagingDirectory"
        New-Item -ItemType Directory -Path $StagingDirectory | Out-Null
    } elseif ((Get-Item $StagingDirectory).PSIsContainer -eq $false) {
        Write-Error "Specified StagingDirectory path exists but is not a directory: $StagingDirectory"
        exit 1
    }

    # Create placeholder native files for all RIDs
    Write-Host "Creating placeholder native artifacts structure..."
    foreach ($rid in $rids) {
        $nativeDir = Join-Path $StagingDirectory $rid "native"
        # Create RID/native structure, removing existing content if necessary
        if (Test-Path $nativeDir) {
            Remove-Item -Recurse -Force $nativeDir
        }
        New-Item -ItemType Directory -Path $nativeDir -Force | Out-Null
        
        $binaries = Get-NativeBinaryNames $rid
        $libPath = Join-Path $nativeDir $binaries.Lib
        $toolPath = Join-Path $nativeDir $binaries.Tool

        # Create empty files
        if ($binaries.Lib) {
             New-Item -ItemType File -Path $libPath -Force | Out-Null 
        }
        if ($binaries.Tool) {
            New-Item -ItemType File -Path $toolPath -Force | Out-Null
        }
        # Optional: Add verbose logging
        # Write-Host "  Created placeholders for $rid in $nativeDir"
    }
    Write-Host "Finished creating placeholders in $StagingDirectory." -ForegroundColor Green

} catch {
    Write-Error "Failed to create placeholder structure: $($_.Exception.Message)"
    exit 1
} 
