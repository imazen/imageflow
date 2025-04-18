# PowerShell script to pack the full solution, run final test, and conditionally publish to NuGet

param(
    # The version for all NuGet packages
    [Parameter(Mandatory=$true)]
    [string]$PackageVersion,
    # The version constraint for Imageflow.Net dependency
    [Parameter(Mandatory=$false)]
    [string]$ImageflowNetVersion = "*-*", # Default to latest
    # The base path where COMBINED native artifacts for ALL RIDs are located (downloaded from build jobs)
    [Parameter(Mandatory=$true)]
    [string]$CombinedNativeArtifactBasePath,
    # API Key for NuGet.org
    [Parameter(Mandatory=$false)]
    [string]$NuGetApiKey,
    # Source URL for NuGet.org
    [Parameter(Mandatory=$false)]
    [string]$NuGetSourceUrl = "https://api.nuget.org/v3/index.json",
    # API Key (GitHub Token) for GitHub Packages
    [Parameter(Mandatory=$false)]
    [string]$GitHubApiKey, # Typically secrets.GITHUB_TOKEN
    # Source URL for GitHub Packages (Requires Org/User)
    [Parameter(Mandatory=$false)]
    [string]$GitHubSourceUrl,
    # Switch to enable pushing to NuGet.org
    [Parameter(Mandatory=$false)]
    [switch]$PushToNuGet = $false,
    # Switch to enable pushing to GitHub Packages
    [Parameter(Mandatory=$false)]
    [switch]$PushToGitHub = $false,
    # Switch to delete pushed packages if any push fails
    [Parameter(Mandatory=$false)]
    [switch]$DeleteOnFailure = $true,
    # Optional: Specify the configuration (Default: Release)
    [Parameter(Mandatory=$false)]
    [string]$Configuration = "Release",
    # Optional: Specify a RID for the final test run (Default: host RID or linux-x64)
    [Parameter(Mandatory=$false)]
    [string]$TestRid
)

$ErrorActionPreference = "Stop"

# --- Helper Functions ---
function Get-HostRid {
    # (Implementation copied from single-pack-and-test.ps1)
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

    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { $arch = "x64" }
        "ARM64" { $arch = "arm64" }
        "x86"   { $arch = "x86" }
        default {
            Write-Warning "Could not determine Processor Architecture from `$env:PROCESSOR_ARCHITECTURE: $($env:PROCESSOR_ARCHITECTURE)"
            return $null
        }
    }

    if ($os -ne "" -and $arch -ne "") {
        return "$($os)-$($arch)"
    } else {
        return $null
    }
}

# --- Script Setup ---

# Get script directory and workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$WorkspaceRoot = Resolve-Path (Join-Path $ScriptDir "..\..") # Assuming script is in dotnet/nuget/

# Define paths
$SolutionFile = Join-Path $WorkspaceRoot "dotnet/nuget/Imageflow.sln"
$EndToEndTestProject = Join-Path $WorkspaceRoot "dotnet/nuget/test/Imageflow.EndToEnd.Test.csproj"
$PackOutputDirectory = Join-Path $WorkspaceRoot "artifacts/nuget" # Final package output location

# Validate inputs
if ($PushToNuGet -and (-not $NuGetApiKey -or $NuGetApiKey -eq '')) {
    Write-Error "NuGetApiKey parameter is required when -PushToNuGet switch is specified."
    exit 1
}
if ($PushToGitHub -and (-not $GitHubApiKey -or $GitHubApiKey -eq '' -or -not $GitHubSourceUrl -or $GitHubSourceUrl -eq '')) {
    Write-Error "GitHubApiKey and GitHubSourceUrl parameters are required when -PushToGitHub switch is specified."
    exit 1
}
if (-not (Test-Path $CombinedNativeArtifactBasePath -PathType Container)) {
    Write-Error "CombinedNativeArtifactBasePath does not exist or is not a directory: $CombinedNativeArtifactBasePath"
    exit 1
}
if (-not (Test-Path $SolutionFile)) {
    Write-Error "Solution file not found at: $SolutionFile"
    exit 1
}
if (-not (Test-Path $EndToEndTestProject)) {
    Write-Error "EndToEnd test project not found at: $EndToEndTestProject"
    exit 1
}

# Determine Test RID if not provided
if (-not $TestRid -or $TestRid -eq '') {
    $TestRid = Get-HostRid
    if (-not $TestRid) {
        Write-Host "Could not determine host RID for test, defaulting to linux-x64" -ForegroundColor Yellow
        $TestRid = "linux-x64"
    }
    Write-Host "TestRid not specified, using: $TestRid" -ForegroundColor Yellow
}

# --- 1. Clean, Restore, and Pack Solution ---
Write-Host "`n--- Packing Solution: $SolutionFile ---" -ForegroundColor Yellow

# Clean previous pack output (optional but recommended)
if (Test-Path $PackOutputDirectory) {
    Write-Host "Cleaning previous pack output directory: $PackOutputDirectory"
    Remove-Item -Recurse -Force $PackOutputDirectory
}
New-Item -ItemType Directory -Path $PackOutputDirectory | Out-Null

try {
    # Ensure we are in the workspace root for the pack relative paths
    Set-Location $WorkspaceRoot

    # Clean first
    Write-Host "`nCleaning solution $SolutionFile ..."
    dotnet clean $SolutionFile -c $Configuration

    # # Optional: More aggressive clean (if needed)
    # Remove-Item -Recurse -Force $WorkspaceRoot/dotnet/nuget/**/bin -ErrorAction SilentlyContinue
    # Remove-Item -Recurse -Force $WorkspaceRoot/dotnet/nuget/**/obj -ErrorAction SilentlyContinue

    # Restore solution first, forcing evaluation for ImageflowNetVersion
    Write-Host "`nRestoring solution packages (forcing evaluation)..."
    dotnet restore $SolutionFile --force-evaluate /p:ImageflowNetVersion=$ImageflowNetVersion
    if ($LASTEXITCODE -ne 0) {
        Write-Error "❌ Restore Solution FAILED with exit code $LASTEXITCODE" -ForegroundColor Red
        exit 1
    }

    Write-Host "`nPacking solution $SolutionFile ..."
    Write-Host "  PackageVersion: $PackageVersion"
    Write-Host "  ImageflowNetVersion: $ImageflowNetVersion"
    Write-Host "  CombinedNativeArtifactBasePath: $CombinedNativeArtifactBasePath"
    Write-Host "  Output Directory: $PackOutputDirectory"

    # Use -o to specify output directory explicitly
    # Pass NativeArtifactBasePath for projects to find their RID-specific native files
    dotnet pack $SolutionFile -c $Configuration -o $PackOutputDirectory `
        /p:Version=$PackageVersion `
        /p:ImageflowNetVersion=$ImageflowNetVersion `
        /p:NativeArtifactBasePath=$CombinedNativeArtifactBasePath `
        --no-restore # Already restored

    if ($LASTEXITCODE -ne 0) {
        Write-Error "❌ Pack Solution FAILED: $($_.Exception.Message)" -ForegroundColor Red
        exit 1
    }

    Write-Host "✅ Pack Solution SUCCEEDED." -ForegroundColor Green

} catch {
    $lastError = $Error[0]
    # Write primary error message to host (NO COLOR)
    Write-Host "Pack Solution FAILED: $($lastError.Exception.Message)"
    # Write minimal error to set $ErrorActionPreference behavior
    Write-Error "Pack Solution FAILED."
    exit 1
}


# --- 2. Build and Run Final EndToEnd Test App --- 
Write-Host "`n--- Building and Running Final EndToEnd Test App (RID: ${TestRid}) ---" -ForegroundColor Yellow
try {
    Write-Host "Restoring final test project ($EndToEndTestProject) with final packed sources..."

    # Explicitly restore first with --force-evaluate
    dotnet restore $EndToEndTestProject `
        -r $TestRid `
        -s $PackOutputDirectory `
        -s "https://api.nuget.org/v3/index.json" `
        /p:RuntimePackageVersion=$PackageVersion `
        /p:ImageflowNetVersion=$ImageflowNetVersion `
        --force-evaluate

    if ($LASTEXITCODE -ne 0) {
        Write-Error "❌ Restoring final $EndToEndTestProject FAILED." -ForegroundColor Red
        exit 1
    }

    Write-Host "Building final test project ($EndToEndTestProject)..."
    # Build the console app using the final packed nugets and --no-restore
    dotnet build $EndToEndTestProject `
        -c $Configuration -r $TestRid `
        --source $PackOutputDirectory `
        --source "https://api.nuget.org/v3/index.json" `
        /p:RuntimePackageVersion=$PackageVersion `
        /p:ImageflowNetVersion=$ImageflowNetVersion `
        --force-evaluate --no-restore # Ensure correct versions are used

    if ($LASTEXITCODE -ne 0) {
        Write-Error "❌ Building final $EndToEndTestProject FAILED." -ForegroundColor Red
        exit 1
    }

    # Construct path to the built executable (similar logic to single-pack script)
    $testExeDir = Join-Path $WorkspaceRoot "dotnet/nuget/test/bin/$Configuration/net8.0/$TestRid/publish"
    $testExeName = (Get-Item (Join-Path $WorkspaceRoot "dotnet/nuget/test/Imageflow.EndToEnd.Test.csproj")).BaseName
    if ($IsWindows) { $testExeName += ".exe" }
    $testExePath = Join-Path $testExeDir $testExeName

    # Fallback path if not published self-contained
    if (-not (Test-Path $testExePath)){
         $testExeDir = Join-Path $WorkspaceRoot "dotnet/nuget/test/bin/$Configuration/net8.0/$TestRid"
         $testExePath = Join-Path $testExeDir $testExeName
    }

    if (-not (Test-Path $testExePath)) {
        Write-Error "❌ Final Test executable not found after build at expected path: $testExePath" -ForegroundColor Red
        exit 1
    }

    Write-Host "Running final test executable: $testExePath ..."
    # Run the executable directly
    & $testExePath
    $exitCode = $LASTEXITCODE

    if ($exitCode -ne 0) {
        Write-Error "❌ Final EndToEnd Test App FAILED for RID ${TestRid} with exit code $exitCode" -ForegroundColor Red
        exit 1
    }

    Write-Host "✅ Final EndToEnd Test App SUCCEEDED for RID ${TestRid}." -ForegroundColor Green

} catch {
    $lastError = $Error[0]
    # Write primary error message to host (NO COLOR)
    Write-Host "Final EndToEnd Test App Build/Run FAILED for RID ${TestRid}: $($lastError.Exception.Message)"
    # Write minimal error to set $ErrorActionPreference behavior
    Write-Error "Final EndToEnd Test App Build/Run FAILED for RID ${TestRid}."
    exit 1
}


# --- 3. Publish Packages (Conditional) --- 
if (-not $PushToNuGet -and -not $PushToGitHub) {
    Write-Host "`nSkipping NuGet/GitHub push (Push switches not specified)." -ForegroundColor Yellow
    Write-Host "`nScript completed successfully (Pack and Test only)." -ForegroundColor Green
    exit 0
}

Write-Host "`n--- Pushing Packages ---`n" -ForegroundColor Yellow

$nupkgs = Get-ChildItem -Path $PackOutputDirectory -Filter *.nupkg -Recurse
if ($nupkgs.Count -eq 0) {
    Write-Error "No .nupkg files found in $PackOutputDirectory to push."
    exit 1
}

# Store push results for potential rollback
$pushResults = @{}
# Format: $pushResults."NuGet.org" = @{ Pushed = @(); Failed = @() }
#         $pushResults."GitHub"    = @{ Pushed = @(); Failed = @() }

function Invoke-Push {
    param(
        [string]$TargetName, # "NuGet.org" or "GitHub"
        [string]$SourceUrl,
        [string]$ApiKey,
        [switch]$IsEnabled,
        [System.IO.FileInfo]$NupkgFile
    )
    if (-not $IsEnabled) { return }

    Write-Host "Pushing $($NupkgFile.Name) to $TargetName..." -NoNewline
    try {
        dotnet nuget push $NupkgFile.FullName --api-key $ApiKey --source $SourceUrl --skip-duplicate
        Write-Host " OK" -ForegroundColor Green
        # Record success
        if (-not $pushResults.ContainsKey($TargetName)) { $pushResults[$TargetName] = @{ Pushed = [System.Collections.Generic.List[object]]::new(); Failed = [System.Collections.Generic.List[object]]::new() } }
        $pushResults[$TargetName].Pushed.Add(@{ Name = $NupkgFile.Name; FullPath = $NupkgFile.FullName })
    } catch {
        Write-Host " FAILED" -ForegroundColor Red
        $errorMessage = $_.Exception.Message -replace "`n"," " -replace "`r"," " # Clean error message
        Write-Warning "Failed to push $($NupkgFile.Name) to ${TargetName}: $errorMessage"
        # Record failure
        if (-not $pushResults.ContainsKey($TargetName)) { $pushResults[$TargetName] = @{ Pushed = [System.Collections.Generic.List[object]]::new(); Failed = [System.Collections.Generic.List[object]]::new() } }
        $pushResults[$TargetName].Failed.Add(@{ Name = $NupkgFile.Name; Error = $errorMessage })
    }
}

# Iterate and push to enabled targets
foreach ($nupkg in $nupkgs) {
    Invoke-Push -TargetName "NuGet.org" -SourceUrl $NuGetSourceUrl -ApiKey $NuGetApiKey -IsEnabled $PushToNuGet -NupkgFile $nupkg
    Invoke-Push -TargetName "GitHub" -SourceUrl $GitHubSourceUrl -ApiKey $GitHubApiKey -IsEnabled $PushToGitHub -NupkgFile $nupkg
}

# Check for failures
$anyPushFailed = $false
foreach ($targetName in $pushResults.Keys) {
    if ($pushResults[$targetName].Failed.Count -gt 0) {
        $anyPushFailed = $true
        Write-Error "$($pushResults[$targetName].Failed.Count) package(s) failed to push to $targetName."
    }
}

# --- 4. Delete on Failure (Conditional) --- 
if ($anyPushFailed) {
    if ($DeleteOnFailure) {
        Write-Host "`n--- Attempting to Delete Successfully Pushed Packages Due to Failure ---`n" -ForegroundColor Yellow

        # Regex to extract ID and Version
        $regex = "^(?<id>.+?)\.(?<ver>\d+\.\d+\.\d+(-[\w.-]+)?(?:\+[\w.-]+)?)\.nupkg$"

        foreach ($targetName in $pushResults.Keys) {
            $sourceUrl = if ($targetName -eq "NuGet.org") { $NuGetSourceUrl } else { $GitHubSourceUrl }
            $apiKey = if ($targetName -eq "NuGet.org") { $NuGetApiKey } else { $GitHubApiKey }
            $successfullyPushed = $pushResults[$targetName].Pushed

            if ($successfullyPushed.Count -eq 0) {
                Write-Host "No packages were successfully pushed to $targetName, nothing to delete."
                continue
            }

            Write-Host "Attempting to delete $($successfullyPushed.Count) package(s) from $targetName..."
            foreach ($pushedItem in $successfullyPushed) {
                $pushedFile = $pushedItem.Name
                if ($pushedFile -match $regex) {
                    $packageIdToDelete = $Matches.id
                    $packageVersionToDelete = $Matches.ver
                    Write-Host "  Deleting $packageIdToDelete version $packageVersionToDelete ..." -NoNewline
                    try {
                        dotnet nuget delete $packageIdToDelete $packageVersionToDelete --api-key $apiKey --source $sourceUrl --non-interactive
                        Write-Host " OK" -ForegroundColor Green
                    } catch {
                        Write-Host " FAILED" -ForegroundColor Red
                        $errorMessage = $_.Exception.Message -replace "`n"," " -replace "`r"," "
                        Write-Warning "Failed to delete $packageIdToDelete v$packageVersionToDelete from ${targetName}: $errorMessage"
                        # Continue trying to delete others
                    }
                } else {
                    Write-Warning "Could not parse package ID and version from filename to delete: $pushedFile"
                }
            }
        }
    } else {
        Write-Warning "`nDeleteOnFailure is false, leaving successfully pushed packages on the feed despite failures."
    }
    # Exit with error because the overall push operation failed
    Write-Error "❌ `nOne or more packages failed to push. See warnings above." -ForegroundColor Red
    exit 1
} else {
    Write-Host "`nAll packages pushed successfully to enabled targets." -ForegroundColor Green
    Write-Host "`nScript completed successfully (Pack, Test, and Push)." -ForegroundColor Green
    exit 0
} 
