# PowerShell script to pack the solution using real artifacts and optionally push to NuGet

param(
    [Parameter(Mandatory=$true)]
    [string]$PackageVersion,
    [Parameter(Mandatory=$true)]
    [string]$ImageflowNetVersion,
    [Parameter(Mandatory=$true)]
    [string]$NativeArtifactBasePath, # Path to REAL native artifacts
    [Parameter(Mandatory=$false)]
    [string]$NuGetApiKey, # Required if PushToNuGet is true
    [Parameter(Mandatory=$false)]
    [string]$NuGetSourceUrl = "https://api.nuget.org/v3/index.json",
    [switch]$PushToNuGet = $false,
    [switch]$DeleteOnFailure = $true # Only relevant if PushToNuGet is true
)

$ErrorActionPreference = "Stop"

# --- Validate Inputs --- 
if ($PushToNuGet -and (-not $NuGetApiKey -or $NuGetApiKey -eq '')) {
    Write-Error "NuGetApiKey parameter is required when PushToNuGet switch is specified."
    exit 1
}
if (-not (Test-Path $NativeArtifactBasePath -PathType Container)) {
    Write-Error "NativeArtifactBasePath does not exist or is not a directory: $NativeArtifactBasePath"
    exit 1
}

# Get script directory and workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
# Assuming script is in dotnet/, workspace root is one level up
$WorkspaceRoot = Resolve-Path (Join-Path $ScriptDir "..")

$solutionFile = Join-Path $WorkspaceRoot "dotnet/nuget/Imageflow.sln"
$packOutputDirectory = Join-Path $WorkspaceRoot "artifacts/nuget" # Default pack output, ensure it matches common targets

# --- 1. Pack the Solution --- 
Write-Host "--- Packing Solution ---`n" -ForegroundColor Yellow

if (-not (Test-Path $solutionFile)) {
    Write-Error "Solution file not found at $solutionFile"
    exit 1
}

# Clean previous pack output (optional but recommended)
if (Test-Path $packOutputDirectory) {
    Write-Host "Cleaning previous pack output directory: $packOutputDirectory"
    Remove-Item -Recurse -Force $packOutputDirectory
}
New-Item -ItemType Directory -Path $packOutputDirectory | Out-Null

try {
    Write-Host "Running: dotnet pack $solutionFile ..."
    Write-Host "  PackageVersion (Native): $PackageVersion"
    Write-Host "  ImageflowNetVersion: $ImageflowNetVersion"
    Write-Host "  NativeArtifactBasePath: $NativeArtifactBasePath"
    Write-Host "  Output Directory: $packOutputDirectory"

    # Ensure we are in the workspace root for the pack relative paths
    Set-Location $WorkspaceRoot

    # Use -o to specify output directory explicitly, ensuring it matches expectations
    dotnet pack $solutionFile -c Release -o $packOutputDirectory `
        /p:Version=$PackageVersion `
        /p:ImageflowNetVersion=$ImageflowNetVersion `
        /p:NativeArtifactBasePath=$NativeArtifactBasePath

    Write-Host "`nPack Solution SUCCEEDED." -ForegroundColor Green

} catch {
    Write-Error "Pack Solution FAILED: $($_.Exception.Message)"
    exit 1
}

# --- 2. Push to NuGet (Conditional) --- 
if (-not $PushToNuGet) {
    Write-Host "`nSkipping NuGet push (PushToNuGet switch not specified)." -ForegroundColor Yellow
    Write-Host "`nScript completed successfully (Pack only)." -ForegroundColor Green
    exit 0
}

Write-Host "`n--- Pushing Packages to NuGet ---`n" -ForegroundColor Yellow
Write-Host "Source URL: $NuGetSourceUrl"

$nupkgs = Get-ChildItem -Path $packOutputDirectory -Filter *.nupkg -Recurse
if ($nupkgs.Count -eq 0) {
    Write-Error "No .nupkg files found in $packOutputDirectory to push."
    exit 1
}

$pushedPackages = [System.Collections.Generic.List[string]]::new()
$failedPackages = [System.Collections.Generic.List[string]]::new()
$pushFailed = $false

foreach ($nupkg in $nupkgs) {
    $packageName = $nupkg.Name
    Write-Host "Pushing $packageName ..." -NoNewline
    try {
        # Use --skip-duplicate to avoid errors if already pushed (e.g., during retry)
        dotnet nuget push $nupkg.FullName --api-key $NuGetApiKey --source $NuGetSourceUrl --skip-duplicate
        Write-Host " OK" -ForegroundColor Green
        $pushedPackages.Add($nupkg.Name) # Add base name for potential deletion
    } catch {
        Write-Host " FAILED" -ForegroundColor Red
        Write-Warning "Failed to push $($packageName): $($_.Exception.Message)"
        $failedPackages.Add($packageName)
        $pushFailed = $true
    }
}

# --- 3. Delete on Failure (Conditional) --- 
if ($pushFailed) {
    Write-Error "One or more packages failed to push."
    if ($DeleteOnFailure) {
        Write-Host "`n--- Attempting to Delete Successfully Pushed Packages ---`n" -ForegroundColor Yellow
        
        if ($pushedPackages.Count -eq 0) {
            Write-Warning "No packages were successfully pushed, nothing to delete."
        } else {
            # Extract package ID and Version from the filename (e.g., Imageflow.NativeRuntime.win-x64.0.0.1-test.nupkg)
            # Assumes standard SemVer 2.0 compatible versions, may need adjustment
            $regex = "^(?<id>.+?)\.(?<ver>\d+\.\d+\.\d+(-[\w.-]+)?)\.nupkg$"

            foreach ($pushedFile in $pushedPackages) {
                if ($pushedFile -match $regex) {
                    $packageIdToDelete = $Matches.id
                    $packageVersionToDelete = $Matches.ver
                    Write-Host "Deleting $packageIdToDelete version $packageVersionToDelete ..." -NoNewline
                    try {
                        dotnet nuget delete $packageIdToDelete $packageVersionToDelete --api-key $NuGetApiKey --source $NuGetSourceUrl --non-interactive
                        Write-Host " OK" -ForegroundColor Green
                    } catch {
                        Write-Host " FAILED" -ForegroundColor Red
                        $errorMessage = $_.Exception.Message
                        Write-Warning "Failed to delete $packageIdToDelete $packageVersionToDelete: $errorMessage"
                        # Continue trying to delete others
                    }
                } else {
                    Write-Warning "Could not parse package ID and version from filename to delete: $pushedFile"
                }
            }
        }
    } else {
        Write-Warning "DeleteOnFailure is false, leaving successfully pushed packages on the feed."
    }
    # Exit with error because the push operation failed
    exit 1 
} else {
    Write-Host "`nAll packages pushed successfully." -ForegroundColor Green
    Write-Host "`nScript completed successfully (Pack and Push)." -ForegroundColor Green
    exit 0
} 
