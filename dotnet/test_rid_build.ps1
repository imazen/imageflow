# PowerShell script to test building the EndToEnd project for the host RID

param(
    # ImageflowNetVersion is now fetched automatically
)

$ErrorActionPreference = "Stop"

# Get script directory and workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$WorkspaceRoot = Resolve-Path (Join-Path $ScriptDir "..")

# Source utility functions
. (Join-Path $ScriptDir "utils.ps1")

# --- Test: Build EndToEnd Test Project for Host RID --- 
Write-Host "--- Running Test: Build EndToEnd Test for Host RID --- `n" -ForegroundColor Yellow

function Get-HostRid {
    $os = ""
    $arch = ""
    if ($IsWindows) {
        $os = "win"
    } elseif ($IsLinux) {
        $os = "linux"
    } elseif ($IsMacOS) {
        $os = "osx"
    } else {
        Write-Warning "Could not determine OS."
        return $null
    }

    # Check PROCESSOR_ARCHITECTURE first
    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { $arch = "x64" }
        "ARM64" { $arch = "arm64" }
        "x86"   { 
            # On 64-bit Windows running 32-bit PS, PROCESSOR_ARCHITECTURE is x86.
            # Check PROCESSOR_ARCHITEW6432 for the actual underlying arch if needed,
            # but for RID detection, 'x86' is usually correct here.
            $arch = "x86" 
        }
        default {
            Write-Warning "Could not determine Processor Architecture from `$env:PROCESSOR_ARCHITECTURE: $($env:PROCESSOR_ARCHITECTURE)"
            # Maybe check systeminfo or uname -m as a fallback if needed
            return $null
        }
    }
    
    # Combine OS and Arch
    if ($os -ne "" -and $arch -ne "") {
        return "$($os)-$($arch)"
    } else {
        return $null
    }
}

# Fetch latest Imageflow.Net version
$latestImageflowNetVersion = "*-*"

$hostRid = Get-HostRid
$testProject = Join-Path $WorkspaceRoot "dotnet/nuget/test/Imageflow.EndToEnd.Test.csproj"

if (-not $hostRid) {
    Write-Error "Test: Could not determine host RID. Skipping EndToEnd build test."
    exit 1 # Or just skip if preferred
}

if (-not (Test-Path $testProject)) {
    Write-Error "Test: Test project not found at $testProject"
    exit 1
}

try {
    Write-Host "Detected Host RID: $hostRid"
    # Write-Host "Running: dotnet build $testProject -r $hostRid ..."
    Write-Host "  Using ImageflowNetVersion: $latestImageflowNetVersion"

    # Ensure we are in the workspace root for the build relative paths
    Set-Location $WorkspaceRoot
    
    dotnet build $testProject -c Release  /p:ImageflowNetVersion=$latestImageflowNetVersion

    Write-Host "`nTest: Build EndToEnd Test SUCCEEDED for RID $hostRid" -ForegroundColor Green

    # Run the test
    dotnet run -c Release --project $testProject
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Test: EndToEnd Test FAILED with exit code $LASTEXITCODE"
        exit 1
    }
    Write-Host "`nTest: EndToEnd Test SUCCEEDED" -ForegroundColor Green

} catch {
    Write-Error "Test: Build EndToEnd Test FAILED for RID $($hostRid): $($_.Exception.Message)"
    exit 1
}

Write-Host "`nBuild test completed successfully." -ForegroundColor Green 
