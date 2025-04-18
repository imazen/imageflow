#Requires -Version 5.1
<#
.SYNOPSIS
Consolidated script for building, testing, and publishing Imageflow .NET NuGet packages.

.DESCRIPTION
Manages the entire .NET NuGet package build, test, and publish workflow for Imageflow.
Supports different modes for local testing, CI matrix jobs, and final CI aggregation/publishing.

.PARAMETER Mode
Specifies the operational mode:
- SingleTest: Local testing for a single RID. Expects native binaries for TargetRid in NativeArtifactBasePath. Uses placeholders for other RIDs. Packs all, tests TargetRid.
- SingleCI:   CI matrix job testing for a single RID. Identical behavior to SingleTest.
- MultiCI:    Final CI aggregation/publishing. Expects NativeArtifactBasePath to contain subdirs for ALL RIDs with real binaries. Packs all, optionally tests host RID, optionally validates and publishes.

.PARAMETER TargetRid
The specific Runtime Identifier to test against in SingleTest/SingleCI modes. Mandatory for these modes.

.PARAMETER PackageVersion
The version number for the NuGet packages (e.g., "0.13.2").

.PARAMETER ImageflowNetVersion
The version specifier for the Imageflow.Net dependency (e.g., "0.13.*").

.PARAMETER NativeArtifactBasePath
Path to the directory containing native binaries.
- For SingleTest/SingleCI: Contains binaries ONLY for the TargetRid.
- For MultiCI: Contains subdirectories named after each supported RID (e.g., win-x64, linux-x64), each containing the native binaries for that RID.

.PARAMETER PackOutputDirectory
The directory where the generated .nupkg files will be placed.

.PARAMETER Configuration
The build configuration (default: "Release").

.PARAMETER SkipTest
If specified, skips the execution of the end-to-end tests.

.PARAMETER PushToNuGet
If specified (only in MultiCI mode), attempts to push packages to the official NuGet source. Requires NuGetApiKey.

.PARAMETER PushToGitHub
If specified (only in MultiCI mode), attempts to upload packages to the GitHub release. Requires gh CLI and GITHUB_TOKEN.

.PARAMETER DeleteNuGetOnFailure
If specified (only relevant when PushToNuGet is true in MultiCI mode), attempts to delete successfully pushed packages from NuGet if any package fails to push for the current version. Defaults to $true.

.PARAMETER NuGetSourceUrl
The URL for the NuGet package source (default: "https://api.nuget.org/v3/index.json").

.PARAMETER NuGetApiKey
API key for authenticating with the NuGet source.

.EXAMPLE
# Local test for win-x64, assuming binaries are in artifacts/native/current_build
./build-pipeline.ps1 -Mode SingleTest -TargetRid win-x64 -PackageVersion 1.0.0 -ImageflowNetVersion 1.0.* -NativeArtifactBasePath ../artifacts/native/current_build -PackOutputDirectory ../../artifacts/nuget_output

.EXAMPLE
# CI matrix job for linux-arm64
./build-pipeline.ps1 -Mode SingleCI -TargetRid linux-arm64 -PackageVersion 1.0.0 -ImageflowNetVersion 1.0.* -NativeArtifactBasePath ../native_binaries/linux-arm64 -PackOutputDirectory ../../artifacts/nuget_output

.EXAMPLE
# Final CI publish step, assuming downloaded artifacts are in artifacts/native_binaries
./build-pipeline.ps1 -Mode MultiCI -PackageVersion 1.0.0 -ImageflowNetVersion 1.0.* -NativeArtifactBasePath ../native_binaries -PackOutputDirectory ../../artifacts/nuget_output -PushToNuGet -PushToGitHub -NuGetApiKey $env:NUGET_API_KEY -DeleteNuGetOnFailure:$false

#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [ValidateSet('SingleTest', 'SingleCI', 'MultiCI')]
    [string]$Mode,

    [Parameter(Mandatory=$false)]
    [string]$TargetRid,

    [Parameter(Mandatory=$true)]
    [string]$PackageVersion,

    [Parameter(Mandatory=$true)]
    [string]$ImageflowNetVersion, # Should match the version in Imageflow.Net.All.csproj etc.

    [Parameter(Mandatory=$true)]
    [string]$NativeArtifactBasePath,

    [Parameter(Mandatory=$true)]
    [string]$PackOutputDirectory,

    [Parameter(Mandatory=$false)]
    [string]$Configuration = "Release",

    [Parameter(Mandatory=$false)]
    [switch]$SkipTest,

    [Parameter(Mandatory=$false)]
    [switch]$PushToNuGet,

    [Parameter(Mandatory=$false)]
    [switch]$PushToGitHub,

    [Parameter(Mandatory=$false)]
    [bool]$DeleteNuGetOnFailure = $true, # Default to true as requested

    [Parameter(Mandatory=$false)]
    [string]$NuGetSourceUrl = "https://api.nuget.org/v3/index.json",

    [Parameter(Mandatory=$false)]
    [string]$NuGetApiKey
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# --- Helper Functions ---
function Get-HostRid {
    $os = $null
    $arch = $null
    if ($IsWindows) { $os = "win" }
    elseif ($IsMacOS) { $os = "osx" }
    elseif ($IsLinux) { $os = "linux" }
    else { throw "Unsupported OS" }

    $nativeArch = switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { "x64" }
        "ARM64" { "arm64" }
        "x86"   { "x86" } # Should not happen on modern systems where PowerShell Core runs
        default { throw "Unsupported Architecture: $($env:PROCESSOR_ARCHITECTURE)" }
    }
    # Handle potential nuances like musl vs gnu on Linux if needed later
    return "$($os)-$($nativeArch)"
}

function Get-NativeBinaryName {
    param([string]$rid)
    if ($rid.StartsWith("win-")) { return "imageflow.dll" }
    if ($rid.StartsWith("osx-")) { return "libimageflow.dylib" }
    if ($rid.StartsWith("linux-")) { return "libimageflow.so" }
    throw "Cannot determine native binary name for RID: $rid"
}

function Get-NativeToolName {
    param([string]$rid)
    if ($rid.StartsWith("win-")) { return "imageflow_tool.exe" }
    return "imageflow_tool" # Linux, macOS
}

function Get-RidSpecificProjectName {
    param([string]$rid)
    # No longer mapping needed here, the project file uses the standard RID
    # switch ($rid) {
    #     'win-x64' { return "win-x86_64" }
    #     'osx-x64' { return "osx-x86_64" }
    #     default { return $rid }
    # }
    return $rid # Project file name uses the standard RID
}

# New function to get the suffix used in the RID-specific package name
function Get-RidSpecificPackageNameSuffix {
     param([string]$rid)
    # Handle known mappings where the .nupkg name suffix differs from the nuget RID
    # Based on ci.yml matrix.package-suffix where it differs from matrix.nuget-rid
    switch ($rid) {
        'win-x64' { return "win-x86_64" }
        'osx-x64' { return "osx-x86_64" }
        default { return $rid } # Default: assume package name suffix matches RID
    }
}

function Write-HostVerbose {
    param([string]$Message)
    Write-Host "VERBOSE: $Message" -ForegroundColor Gray
}

function Write-HostWarning {
    param([string]$Message)
    Write-Warning $Message
}

function Write-HostError {
    param([string]$Message)
    Write-Error $Message
}

# Helper function to run diagnostics on test failure
function Invoke-TestDiagnostics {
    param(
        [string]$ridToTest,
        [string]$depsJsonPath, # Might be null
        [string]$IntermediatePackDir,
        [string]$PackageVersion
    )
   # $ErrorActionPreference = "SilentlyContinue" # Allow diagnostics to proceed even if parts fail
    Write-Host "--- Running Test Failure Diagnostics for RID $ridToTest --- START --- " -ForegroundColor Yellow
    $Host.UI.RawUI.FlushInputBuffer() # Try flushing
    Start-Sleep -Milliseconds 100 # Give buffer a moment

    # 1. Show .deps.json (if possible)
    if ($depsJsonPath) {
        Write-Host "--- DEBUG: Contents of '$($depsJsonPath)' ---" -ForegroundColor Magenta
        if (Test-Path $depsJsonPath) {
            try {
                Get-Content $depsJsonPath | Write-Host -ForegroundColor Magenta
            } catch {
                Write-HostError "  Error reading deps.json: $($_.Exception.Message)"
            }
        } else { Write-HostWarning "  File not found or test failed before generation: $depsJsonPath" }
        Write-Host "--- DEBUG END: $depsJsonPath ---" -ForegroundColor Magenta
    } else {
        Write-HostWarning "  Test executable path not determined, cannot show .deps.json (likely build/restore failure)."
    }

    # 2. Inspect relevant Nuget packages
    Write-Host "--- DEBUG: Inspecting potentially relevant NuGet packages from '$IntermediatePackDir' --- " -ForegroundColor Magenta
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem # Ensure loaded
    } catch {
        Write-HostError "  Failed to load System.IO.Compression.FileSystem. Cannot inspect packages: $($_.Exception.Message)"
        return # Exit diagnostic function if compression assembly fails
    }

    # Package Names 
    $packageNameSuffix = Get-RidSpecificPackageNameSuffix -rid $ridToTest
    $ridPackageNamePattern = "Imageflow.NativeRuntime.${packageNameSuffix}.${PackageVersion}.nupkg"
    $allPackageNamePattern = "Imageflow.NativeRuntime.All.${PackageVersion}.nupkg"

    $packagesToInspect = @(
        Get-ChildItem -Path $IntermediatePackDir -Filter $ridPackageNamePattern -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
        Get-ChildItem -Path $IntermediatePackDir -Filter $allPackageNamePattern -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    ) | Where-Object { $_ -ne $null }

    if ($null -eq $packagesToInspect -or $packagesToInspect.Count -eq 0) {
        Write-HostWarning "  Could not find expected packages ($ridPackageNamePattern, $allPackageNamePattern) in '$IntermediatePackDir' to inspect."
    } else {
        foreach ($nupkgFile in $packagesToInspect) {
            Write-Host "--- DEBUG: Contents of $($nupkgFile.Name) --- " -ForegroundColor Magenta
            try {
                $zip = [System.IO.Compression.ZipFile]::OpenRead($nupkgFile.FullName)
                
                # Print directory listing
                Write-Host "    --- File Listing ---" -ForegroundColor Cyan
                $zip.Entries | Sort-Object FullName | ForEach-Object {
                    Write-Host "    $($_.FullName) (Size: $($_.Length))" -ForegroundColor Magenta
                }

                # Extract and print .nuspec content
                Write-Host "    --- Nuspec Content ---" -ForegroundColor Cyan
                $nuspecEntry = $zip.Entries | Where-Object { $_.Name -like '*.nuspec' } | Select-Object -First 1
                if ($nuspecEntry) {
                    try {
                        $stream = $nuspecEntry.Open()
                        $reader = [System.IO.StreamReader]::new($stream)
                        $nuspecContent = $reader.ReadToEnd()
                        $reader.Dispose() # Dispose reader first
                        $stream.Dispose() # Then dispose stream
                        Write-Host $nuspecContent -ForegroundColor Magenta
                    } catch {
                         Write-HostError "      Error reading nuspec content from '$($nuspecEntry.Name)': $($_.Exception.Message)"
                    }
                } else {
                    Write-HostWarning "      Could not find .nuspec file in '$($nupkgFile.Name)'."
                }
                
                $zip.Dispose() # Dispose zip archive
            } catch {
                Write-HostError "  Error inspecting package '$($nupkgFile.Name)': $($_.Exception.Message)"
            }
            Write-Host "--- DEBUG END: $($nupkgFile.Name) --- " -ForegroundColor Magenta
        }
    }
     Write-Host "--- End Test Failure Diagnostics --- END --- " -ForegroundColor Yellow
     $Host.UI.RawUI.FlushInputBuffer() # Try flushing
     Start-Sleep -Milliseconds 100 # Give buffer a moment
     $ErrorActionPreference = "Stop" # Restore preference
}

function Invoke-CommandAndLog {
    param(
        [string]$Executable,
        [string[]]$Arguments,
        [string]$LogIdentifier = ""
    )
    $commandString = "$Executable $($Arguments -join ' ')"
    Write-Host "Executing: $commandString"
    & $Executable $Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $commandString ($LogIdentifier)"
    }
    Write-HostVerbose "Command succeeded: $commandString ($LogIdentifier)"
}


# --- Initial Setup & Validation ---

Write-Host "Starting build-pipeline.ps1 in Mode '$Mode'"
Write-Host "Parameters:"
Write-Host "  Mode: $Mode"
Write-Host "  TargetRid: $($TargetRid | Out-String)"
Write-Host "  PackageVersion: $PackageVersion"
Write-Host "  ImageflowNetVersion: $ImageflowNetVersion"
Write-Host "  NativeArtifactBasePath: $NativeArtifactBasePath"
Write-Host "  PackOutputDirectory: $PackOutputDirectory"
Write-Host "  Configuration: $Configuration"
Write-Host "  SkipTest: $SkipTest"
Write-Host "  PushToNuGet: $PushToNuGet"
Write-Host "  PushToGitHub: $PushToGitHub"
Write-Host "  DeleteNuGetOnFailure: $DeleteNuGetOnFailure"
Write-Host "  NuGetSourceUrl: $NuGetSourceUrl"
Write-Host "  NuGetApiKey Provided: $(!([string]::IsNullOrEmpty($NuGetApiKey)))"


# Validate parameters based on mode
if (($Mode -eq 'SingleTest' -or $Mode -eq 'SingleCI') -and [string]::IsNullOrEmpty($TargetRid)) {
    throw "Parameter -TargetRid is mandatory for Mode '$Mode'."
}
if (($Mode -ne 'MultiCI') -and ($PushToNuGet -or $PushToGitHub)) {
    Write-HostWarning "PushToNuGet and PushToGitHub flags are ignored in Mode '$Mode'. Publishing only occurs in 'MultiCI' mode."
    $PushToNuGet = $false
    $PushToGitHub = $false
}
if ($PushToNuGet -and [string]::IsNullOrEmpty($NuGetApiKey)) {
    throw "Parameter -NuGetApiKey is mandatory when -PushToNuGet is specified."
}
if ($PushToGitHub) {
    # Check if gh CLI is available
    $ghPath = Get-Command gh -ErrorAction SilentlyContinue
    if (-not $ghPath) {
        throw "GitHub CLI ('gh') not found in PATH. It is required for -PushToGitHub."
    }
    # Check for GITHUB_TOKEN environment variable (common practice)
    if ([string]::IsNullOrEmpty($env:GITHUB_TOKEN)) {
        throw "GITHUB_TOKEN environment variable not found or empty. GitHub release upload cannot proceed."
    }
     # Check for GITHUB_REPOSITORY environment variable (needed for gh release upload)
    if ([string]::IsNullOrEmpty($env:GITHUB_REPOSITORY)) {
         throw "GITHUB_REPOSITORY environment variable not found or empty. GitHub release upload cannot proceed."
    }
     # Check for GITHUB_REF environment variable (to get the tag name)
    if ([string]::IsNullOrEmpty($env:GITHUB_REF) -or !$env:GITHUB_REF.StartsWith("refs/tags/")) {
        throw "GITHUB_REF environment variable not found, empty, or not a tag ref ('refs/tags/...'). Cannot determine release tag for GitHub upload."
    }
}


# Define paths relative to the script location ($PSScriptRoot)
$WorkspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path # Assumes script is in dotnet/nuget/scripts
$NugetProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path # dotnet/nuget/
$StagingDir = Join-Path $NugetProjectRoot "temp_staging_for_packing"
# Define the intermediate packing directory (for nuget.config)
$IntermediatePackDir = Join-Path $NugetProjectRoot "local"
# Define the final output directory from parameter
# Construct the full path but don't require it to exist yet.
$PackOutputDirectory = if ([System.IO.Path]::IsPathRooted($PackOutputDirectory)) { $PackOutputDirectory } else { [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot $PackOutputDirectory)) }
$NativeArtifactBasePath = if ([System.IO.Path]::IsPathRooted($NativeArtifactBasePath)) { $NativeArtifactBasePath } else { Resolve-Path (Join-Path $PSScriptRoot $NativeArtifactBasePath) }

$SolutionFile = Join-Path $WorkspaceRoot "Imageflow.sln" # Adjust if needed
$EndToEndTestProject = Join-Path $NugetProjectRoot "test/Imageflow.EndToEnd.Test.csproj" # Adjust if needed

# Define all supported RIDs (ensure this list is accurate and maintained)
$AllRids = @(
    "win-x64",
    "win-x86",
    "win-arm64",
    "linux-x64",
    "linux-arm64",
    # "linux-musl-x64", # Removed as per user feedback
    # "linux-musl-arm64", # Removed as per user feedback
    "osx-x64",
    "osx-arm64"
    # Add/remove RIDs as needed
)
Write-Host "Supported RIDs: $($AllRids -join ', ')"


# Clean previous outputs
Write-Host "Cleaning output directory: $PackOutputDirectory"
if (Test-Path $PackOutputDirectory) {
    Remove-Item -Recurse -Force $PackOutputDirectory
}
New-Item -ItemType Directory -Force $PackOutputDirectory | Out-Null

Write-Host "Cleaning staging directory: $StagingDir"
if (Test-Path $StagingDir) {
    Remove-Item -Recurse -Force $StagingDir
}
New-Item -ItemType Directory -Force $StagingDir | Out-Null

# Clean the intermediate packing directory
Write-Host "Cleaning intermediate packing directory: $IntermediatePackDir"
if (Test-Path $IntermediatePackDir) {
    Remove-Item -Recurse -Force $IntermediatePackDir
}
New-Item -ItemType Directory -Force $IntermediatePackDir | Out-Null


# --- Prepare Staging Directory ---

Write-Host "Preparing staging directory '$StagingDir' for Mode '$Mode'..."

if ($Mode -eq 'SingleTest' -or $Mode -eq 'SingleCI') {
    Write-Host "Mode '$Mode': Creating placeholders for all RIDs, then copying real binaries for '$TargetRid' from '$NativeArtifactBasePath'"

    # 1. Create zero-byte placeholders for all RIDs
    foreach ($rid in $AllRids) {
        $ridStagingPath = Join-Path $StagingDir $rid
        New-Item -ItemType Directory -Force $ridStagingPath | Out-Null
        # Create the 'native' subdirectory
        $nativeSubDir = Join-Path $ridStagingPath "native"
        New-Item -ItemType Directory -Force $nativeSubDir | Out-Null

        $binaryName = Get-NativeBinaryName -rid $rid
        $toolName = Get-NativeToolName -rid $rid

        # Create placeholders inside the 'native' subdir
        New-Item -Path (Join-Path $nativeSubDir $binaryName) -ItemType File -Force | Out-Null
        New-Item -Path (Join-Path $nativeSubDir $toolName) -ItemType File -Force | Out-Null
        Write-HostVerbose "Created placeholders in $nativeSubDir"
    }

    # 2. Overwrite placeholders for the TargetRid with real binaries
    $targetRidStagingPath = Join-Path $StagingDir $TargetRid
    # Target the 'native' subdirectory for destination
    $targetNativeDestDir = Join-Path $targetRidStagingPath "native"

    $sourceBinaryPath = Join-Path $NativeArtifactBasePath (Get-NativeBinaryName -rid $TargetRid)
    $sourceToolPath = Join-Path $NativeArtifactBasePath (Get-NativeToolName -rid $TargetRid)
    # Construct destination paths inside the 'native' subdir
    $destBinaryPath = Join-Path $targetNativeDestDir (Get-NativeBinaryName -rid $TargetRid)
    $destToolPath = Join-Path $targetNativeDestDir (Get-NativeToolName -rid $TargetRid)

    if (-not (Test-Path $sourceBinaryPath)) { throw "Required native binary not found for $TargetRid at $sourceBinaryPath" }
    if (-not (Test-Path $sourceToolPath)) { throw "Required native tool not found for $TargetRid at $sourceToolPath" }

    Copy-Item -Path $sourceBinaryPath -Destination $destBinaryPath -Force
    Copy-Item -Path $sourceToolPath -Destination $destToolPath -Force
    Write-Host "Copied real binaries for '$TargetRid' to '$targetRidStagingPath'"

} elseif ($Mode -eq 'MultiCI') {
    Write-Host "Mode '$Mode': Copying real binaries for ALL RIDs from subdirectories within '$NativeArtifactBasePath'"

    foreach ($rid in $AllRids) {
        $sourceRidPath = Join-Path $NativeArtifactBasePath $rid
        $destRidPath = Join-Path $StagingDir $rid
        New-Item -ItemType Directory -Force $destRidPath | Out-Null
        # Create the 'native' subdirectory in staging
        $destNativeDir = Join-Path $destRidPath "native"
        New-Item -ItemType Directory -Force $destNativeDir | Out-Null

        $binaryName = Get-NativeBinaryName -rid $rid
        $toolName = Get-NativeToolName -rid $rid
        $sourceBinary = Join-Path $sourceRidPath $binaryName
        $sourceTool = Join-Path $sourceRidPath $toolName
        # Construct destination paths inside the 'native' subdir
        $destBinary = Join-Path $destNativeDir $binaryName
        $destTool = Join-Path $destNativeDir $toolName

        if (-not (Test-Path $sourceBinary)) { throw "Required native binary not found for $rid at $sourceBinary" }
        if (-not (Test-Path $sourceTool)) { throw "Required native tool not found for $rid at $sourceTool" }

        Copy-Item -Path $sourceBinary -Destination $destBinary -Force
        Copy-Item -Path $sourceTool -Destination $destTool -Force
        Write-HostVerbose "Copied binaries for '$rid' from '$sourceRidPath' to '$destNativeDir'"
    }
    Write-Host "Finished copying all real binaries for MultiCI mode."
} else {
    throw "Invalid Mode '$Mode' encountered during staging preparation."
}

Write-Host "Staging directory preparation complete."
# Optional: List contents for verification
# Get-ChildItem $StagingDir -Recurse


# --- Pack All Packable Projects in Solution --- 

Write-Host "Packing all packable projects in solution '$SolutionFile'..."
# Note: dotnet pack on a solution builds and packs packable projects.
# We pass parameters needed by various projects within the solution.
$solutionPackArgs = @(
    "pack",
    "$SolutionFile",
    "--configuration", $Configuration,
    "--output", $IntermediatePackDir,
    "/p:PackageVersion=$PackageVersion",
    "/p:ImageflowNetVersion=$ImageflowNetVersion", # Needed by Imageflow.Net.All
    "/p:NativeArtifactBasePath=$StagingDir",      # Needed by Imageflow.NativeRuntime.* projects
    "/verbosity:minimal" # Use normal/detailed/diag for debugging
)
Invoke-CommandAndLog -Executable "dotnet" -Arguments $solutionPackArgs -LogIdentifier "Pack Solution"
Write-Host "Finished packing solution projects."


# --- Run Test ---

if ($SkipTest) {
    Write-Host "Skipping tests as requested by -SkipTest flag."
} else {
    Write-Host "Running end-to-end tests..."
    $ridToTest = $null
    if ($Mode -eq 'SingleTest' -or $Mode -eq 'SingleCI') {
        $ridToTest = $TargetRid
    } elseif ($Mode -eq 'MultiCI') {
        $ridToTest = Get-HostRid
        Write-Host "MultiCI mode: Testing against host RID '$ridToTest'."
        if ($ridToTest -notin $AllRids) {
             Write-HostWarning "Host RID '$ridToTest' is not in the list of officially supported/built RIDs. Test might reflect behavior specific to this environment."
        }
    } else {
         throw "Invalid Mode '$Mode' encountered during test preparation."
    }

    if (-not (Test-Path $EndToEndTestProject)) {
         throw "End-to-end test project not found at '$EndToEndTestProject'"
    }

    Write-Host "Attempting to run test for RID: $ridToTest"
    $testProjectDir = Split-Path $EndToEndTestProject -Parent
    $testDepsJsonPath = $null # Define scope outside try
    $testExePath = $null

    # Flag to track if test phase fails, to ensure diagnostics run before throwing
    $script:testFailed = $false 

    $testBuildDir = Join-Path $testProjectDir "run/bin/$ridToTest"

    try {
        # First, delete the test build directory
        if (Test-Path $testBuildDir) {
            Remove-Item -Recurse -Force $testBuildDir
        }

        Write-Host "Building test project '$EndToEndTestProject'..."
        $buildArgs = @(
            "build",
            "$EndToEndTestProject",
            "--configuration", $Configuration,
            "--runtime", $ridToTest,
            "--output", $testBuildDir,
            # Pass the pipeline's PackageVersion to override the default in csproj
            "/p:RuntimePackageVersion=$PackageVersion",
            # Increase verbosity to diagnose missing runtime assets
            "/verbosity:normal" # Or use 'detailed' or 'diag'
        )
        Invoke-CommandAndLog -Executable "dotnet" -Arguments $buildArgs -LogIdentifier "Build Test $ridToTest"

        # 3. Find and Execute test
        $testExeNameWithExe = ($EndToEndTestProject | Split-Path -Leaf).Replace(".csproj", ".exe")
        $testDepsJsonPath = Join-Path $testBuildDir ($testExeNameWithExe.Replace(".exe", "") + ".deps.json")
  
        # linux/mac ext 
        if ($ridToTest -like "*linux*" -or $ridToTest -like "*osx*") {
            $testExePath = Join-Path $testBuildDir $testExeNameWithExe.Replace(".exe", "")
        } else {
            $testExePath = Join-Path $testBuildDir $testExeNameWithExe
        }

        if (-not (Test-Path $testExePath)) {
            Write-HostError "Compiled test executable not found at expected path: $testExePath"
            # Ensure \$foundExes is always an array
            $foundExes = @(Get-ChildItem -Path $publishDir -Filter ($testExeNameWithExe.Replace(".exe", "*")) -File)
            if ($null -ne $foundExes -and $foundExes.Count -eq 1) {
                $testExePath = $foundExes[0].FullName
                Write-HostWarning "Used fallback to find test executable: $testExePath"
            } else {
                # Throw before trying to execute if not found
                throw "Could not reliably find test executable in $publishDir"
            }
        }

        # --- Search recursively for native DLLs in $testBuildDir --- 
        $nativeBinaryName = Get-NativeBinaryName -rid $ridToTest
        # Ensure $nativeDlls is always an array, even if 0 or 1 items are found
        $nativeDlls = @(Get-ChildItem -Path $testBuildDir -Recurse -Filter $nativeBinaryName -File)
        if ($null -eq $nativeDlls -or $nativeDlls.Count -eq 0) {
            $script:testFailed = $true
            Write-Host "Required native DLL ($nativeBinaryName) not found anywhere in $testBuildDir" -ForegroundColor Red
            Invoke-TestDiagnostics -ridToTest $ridToTest -depsJsonPath $testDepsJsonPath -IntermediatePackDir $IntermediatePackDir -PackageVersion $PackageVersion
            Write-HostError "Required native DLL ($nativeBinaryName) not found anywhere in $testBuildDir"
        }

        Write-Host "Executing test: $testExePath"
        try{
            & $testExePath
        } catch {
            Write-Host "$testExeName FAILED" -ForegroundColor Red
            $script:testFailed = $true 
            Invoke-TestDiagnostics -ridToTest $ridToTest -depsJsonPath $testDepsJsonPath -IntermediatePackDir $IntermediatePackDir -PackageVersion $PackageVersion
            Write-HostError "$testExeName FAILED"
        }
        # Only print success if the flag wasn't set
        if (-not $script:testFailed) {
            Write-Host "End-to-end test completed successfully."
        }

    } catch { 
        Write-HostError "Test phase FAILED for RID $ridToTest. Error: $($_.Exception.Message)"
        Invoke-TestDiagnostics -ridToTest $ridToTest -depsJsonPath $testDepsJsonPath -IntermediatePackDir $IntermediatePackDir -PackageVersion $PackageVersion 
        # Set flag instead of throwing immediately
        $script:testFailed = $true
    }

    # Check the flag after the try/catch block and throw if necessary
    if ($script:testFailed) {
        throw "Test phase failed. See diagnostic output above."
    }
}


# --- Validate Packages (MultiCI) ---
$validationFailed = $false
if ($Mode -eq 'MultiCI') {
    Write-Host "Validating generated .nupkg files for zero-byte native binaries..."
    # Requires a tool/method to inspect .nupkg contents (which are zip files)
    # Using System.IO.Compression (PowerShell Core 6+ or .NET Framework 4.5+)
    Add-Type -AssemblyName System.IO.Compression.FileSystem

    # Validate packages in the intermediate directory
    $nupkgFiles = Get-ChildItem -Path $IntermediatePackDir -Filter *.nupkg -Recurse

    foreach ($nupkgFile in $nupkgFiles) {
        Write-HostVerbose "Validating $($nupkgFile.Name)..."
        try {
            $zip = [System.IO.Compression.ZipFile]::OpenRead($nupkgFile.FullName)
            foreach ($entry in $zip.Entries) {
                # Check files within runtime-specific folders that look like our native binaries
                if ($entry.FullName -match '^runtimes/.*/native/.*\.(dll|so|dylib|exe)$') {
                     # Check for our specific names to be more precise
                     $expectedBinaryName = Get-NativeBinaryName -rid ($entry.FullName -replace '^runtimes/(.*?)/.*','$1')
                     $expectedToolName = Get-NativeToolName -rid ($entry.FullName -replace '^runtimes/(.*?)/.*','$1')
                     $actualFileName = $entry.Name

                     if (($actualFileName -eq $expectedBinaryName -or $actualFileName -eq $expectedToolName) -and $entry.Length -eq 0) {
                         Write-HostError "Validation FAILED: Found zero-byte native binary '$($entry.FullName)' in '$($nupkgFile.Name)'."
                         $validationFailed = $true
                         # Optionally break inner loop: break
                     } else {
                          Write-HostVerbose "  OK: $($entry.FullName) (Size: $($entry.Length))"
                     }
                }
            }
            $zip.Dispose()
        } catch {
             Write-HostError "Error validating '$($nupkgFile.Name)': $_"
             $validationFailed = $true # Treat errors during validation as failure
        }
        if ($validationFailed) {
            # Optionally break outer loop: break
        }
    }

    if ($validationFailed) {
        throw "NuGet package validation failed. Found zero-byte native binaries in one or more packages. Aborting publish."
    } else {
        Write-Host "NuGet package validation successful."
    }
}


# --- Publish ---

if ($Mode -eq 'MultiCI') {
    $pushedNuGetPackages = [System.Collections.Generic.List[string]]::new()
    $failedNuGetPackages = [System.Collections.Generic.List[string]]::new()

    if ($PushToNuGet) {
        Write-Host "Publishing packages to NuGet ($NuGetSourceUrl)..."
        # Push packages from the intermediate directory
        # Ensure \$nupkgFilesToPush is always an array
        $nupkgFilesToPush = @(Get-ChildItem -Path $IntermediatePackDir -Filter *.nupkg -Recurse)
        if ($null -eq $nupkgFilesToPush -or $nupkgFilesToPush.Count -eq 0) {
             throw "No .nupkg files found in intermediate directory '$IntermediatePackDir'. Cannot push to NuGet."
        } else {
            foreach ($nupkgFile in $nupkgFilesToPush) {
                Write-Host "Pushing $($nupkgFile.FullName)..."
                $pushArgs = @(
                    "nuget", "push",
                    "$($nupkgFile.FullName)",
                    "--api-key", $NuGetApiKey,
                    "--source", $NuGetSourceUrl,
                    "--skip-duplicate" # Don't fail if already exists
                    #"--timeout", "300" # Optional timeout
                )
                try {
                     Invoke-CommandAndLog -Executable "dotnet" -Arguments $pushArgs -LogIdentifier "Push NuGet $($nupkgFile.Name)"
                     $pushedNuGetPackages.Add($nupkgFile.Name.Replace(".nupkg", "")) # Store base name like Imageflow.NativeRuntime.win-x64
                } catch {
                    Write-HostError "Failed to push NuGet package '$($nupkgFile.Name)': $_"
                    $failedNuGetPackages.Add($nupkgFile.Name.Replace(".nupkg", ""))
                }
            }

            if ($failedNuGetPackages.Count -gt 0) {
                 Write-HostError "Failed to push the following NuGet packages: $($failedNuGetPackages -join ', ')"
                 if ($DeleteNuGetOnFailure -and $pushedNuGetPackages.Count -gt 0) {
                    Write-HostWarning "Attempting to delete successfully pushed packages due to failures and -DeleteNuGetOnFailure flag..."
                    foreach ($packageName in $pushedNuGetPackages) {
                         Write-HostWarning "Deleting $packageName version $PackageVersion from $NuGetSourceUrl..."
                         $deleteArgs = @(
                             "nuget", "delete", "$packageName", "$PackageVersion",
                             "--api-key", $NuGetApiKey,
                             "--source", $NuGetSourceUrl,
                             "--non-interactive"
                         )
                         try {
                              Invoke-CommandAndLog -Executable "dotnet" -Arguments $deleteArgs -LogIdentifier "Delete NuGet $packageName $PackageVersion"
                         } catch {
                             Write-HostError "Failed to delete NuGet package '$packageName $PackageVersion': $_"
                             # Continue trying to delete others
                         }
                    }
                 }
                 # Ensure script exits with error after handling deletion attempt
                 throw "One or more NuGet packages failed to publish."
            } else {
                 Write-Host "All NuGet packages pushed successfully."
            }
        }
    } else {
        Write-Host "Skipping NuGet publish because -PushToNuGet was not specified."
    }

    if ($PushToGitHub) {
        Write-Host "Publishing packages to GitHub Release..."
        # Upload packages from the intermediate directory
        # Ensure \$nupkgFilesToUpload is always an array
        $nupkgFilesToUpload = @(Get-ChildItem -Path $IntermediatePackDir -Filter *.nupkg -Recurse)
        if ($null -eq $nupkgFilesToUpload -or $nupkgFilesToUpload.Count -eq 0) {
             throw "No .nupkg files found in intermediate directory '$IntermediatePackDir'. Cannot upload to GitHub Release."
        } else {
            Write-Host "Uploading $($nupkgFilesToUpload.Count) packages to GitHub release tag '$releaseTag' in repo '$repo'..."
            $uploadArgs = @(
                "release", "upload",
                "$releaseTag",
                $nupkgFilesToUpload.FullName, # Pass array of full paths
                "--repo", $repo,
                "--clobber" # Overwrite if files with same name exist
            )
            # Assumes gh is authenticated via GITHUB_TOKEN env var
            try {
                Invoke-CommandAndLog -Executable "gh" -Arguments $uploadArgs -LogIdentifier "GH Release Upload $releaseTag"
                Write-Host "Successfully uploaded packages to GitHub release '$releaseTag'."
            } catch {
                # Throw here to ensure the overall script fails if GH upload fails
                throw "Failed to upload packages to GitHub release '$releaseTag': $_"
            }
        }
    } else {
        Write-Host "Skipping GitHub release upload because -PushToGitHub was not specified."
    }

} else {
    Write-Host "Skipping publish step because Mode is not 'MultiCI'."
}


# --- Final Copy to Output Directory (if successful) ---
Write-Host "Copying packages from intermediate directory '$IntermediatePackDir' to final output '$PackOutputDirectory'..."
# Ensure the final destination directory exists
if (-not (Test-Path $PackOutputDirectory)) {
    New-Item -ItemType Directory -Force $PackOutputDirectory | Out-Null
}
# Copy all items from intermediate to final. This runs only if no exceptions occurred before this point.
Copy-Item -Path (Join-Path $IntermediatePackDir "*") -Destination $PackOutputDirectory -Recurse -Force
Write-Host "Packages successfully copied to final destination."


# --- Cleanup ---
Write-Host "Cleaning up temporary staging directory: $StagingDir"
if (Test-Path $StagingDir) {
    Remove-Item -Recurse -Force $StagingDir
}

Write-Host "Build pipeline script completed successfully."
exit 0 # Explicit success exit code 
