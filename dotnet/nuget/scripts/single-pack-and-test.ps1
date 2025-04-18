# PowerShell script to pack RID-specific runtime/tool packages and run EndToEnd test

param(
    # The Runtime Identifier (RID) to build for (e.g., win-x64, linux-arm64)
    [Parameter(Mandatory=$false)]
    [string]$RID,
    # The version for the native NuGet packages
    [Parameter(Mandatory=$true)]
    [string]$PackageVersion,
    # The version constraint for Imageflow.Net dependency
    [Parameter(Mandatory=$true)]
    [string]$ImageflowNetVersion,
    # The base path where the native binaries (.dll, .so, .dylib, .exe) are located
    [Parameter(Mandatory=$true)]
    [string]$NativeArtifactBasePath,
    # The directory where the packed .nupkg files should be placed
    [Parameter(Mandatory=$true)]
    [string]$PackOutputDirectory,
    # Optional: Specify the configuration (Default: Release)
    [Parameter(Mandatory=$false)]
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"

# --- Helper Functions ---
function Get-HostRid {
    # (Implementation copied from deleted test_rid_build.ps1)
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

# Determine RID if not provided
if (-not $RID -or $RID -eq '') {
    $RID = Get-HostRid
    if (-not $RID) {
        Write-Error "Could not determine host RID and none was provided via -RID parameter."
        exit 1
    }
    Write-Host "RID not specified, using host RID: $RID" -ForegroundColor Yellow
}

# Get script directory and workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
# Workspace root is three levels up from dotnet/nuget/scripts/
$WorkspaceRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..") # Use Join-Path for robustness

# Define project paths (relative to workspace root)
$NativeRuntimeProject = Join-Path $WorkspaceRoot "dotnet/nuget/native/Imageflow.NativeRuntime.${RID}.csproj"
# Tool project might not exist for all RIDs (e.g., if static linking)
# $NativeToolProject = Join-Path $WorkspaceRoot "dotnet/nuget/native/Imageflow.NativeTool.${RID}.csproj" 
$EndToEndTestProject = Join-Path $WorkspaceRoot "dotnet/nuget/test/Imageflow.EndToEnd.Test.csproj"

# Validate inputs
if (-not (Test-Path $NativeArtifactBasePath -PathType Container)) {
    Write-Error "NativeArtifactBasePath does not exist or is not a directory: $NativeArtifactBasePath"
    exit 1
}
if (-not (Test-Path $NativeRuntimeProject)) {
    Write-Error "Native Runtime project not found for RID $RID at: $NativeRuntimeProject"
    # For static builds (musl), this is expected, treat as non-fatal warning?
    Write-Warning "If this is a static MUSL build, this might be expected. Continuing..."
    # exit 1 # Or decide if this is fatal
}
if (-not (Test-Path $EndToEndTestProject)) {
    Write-Error "EndToEnd test project not found at: $EndToEndTestProject"
    exit 1
}

# Ensure pack output directory exists and is clean
if (Test-Path $PackOutputDirectory) {
    Write-Host "Cleaning pack output directory: $PackOutputDirectory"
    Remove-Item -Recurse -Force $PackOutputDirectory
}
New-Item -ItemType Directory -Path $PackOutputDirectory | Out-Null

# --- 1. Pack RID-Specific Packages --- 
Write-Host "`n--- Packing Packages for RID $RID ---" -ForegroundColor Yellow
try {
    # Pack Runtime Project (if it exists)
    if (Test-Path $NativeRuntimeProject) {
        Write-Host "Packing Runtime: $NativeRuntimeProject ..."
        dotnet pack $NativeRuntimeProject `
            -c $Configuration -o $PackOutputDirectory `
            /p:Version=$PackageVersion `
            /p:ImageflowNetVersion=$ImageflowNetVersion `
            /p:NativeArtifactBasePath=$NativeArtifactBasePath 
            
        if ($LASTEXITCODE -ne 0) {
            Write-Error "❌ Packing $NativeRuntimeProject FAILED."
            exit 1
        }
    } else {
        Write-Host "Skipping pack for non-existent runtime project: $NativeRuntimeProject (Might be static build)"
    }

    # Pack Tool Project (if it exists) - Add logic if/when tool packages are separated per RID
    # if (Test-Path $NativeToolProject) {
    #     Write-Host "Packing Tool: $NativeToolProject ..."
    #     dotnet pack $NativeToolProject `
    #         -c $Configuration -o $PackOutputDirectory `
    #         /p:Version=$PackageVersion `
    #         /p:ImageflowNetVersion=$ImageflowNetVersion `
    #         /p:NativeArtifactBasePath=$NativeArtifactBasePath `
    #         --no-build
    #         # --force-evaluate 
    # }

    Write-Host "✅ Packing for RID $RID SUCCEEDED." -ForegroundColor Green

    # --- DEBUG: Inspect the created native runtime package ---
    Write-Host "`n--- DEBUG (single-pack): Inspecting created Native Runtime package for $RID ---" -ForegroundColor Magenta
    
    # Correct the package ID pattern based on csproj override
    $PackageIdPattern = "Imageflow.NativeRuntime.${RID}.*.nupkg"
    if ($RID -eq 'win-x64') {
        $PackageIdPattern = "Imageflow.NativeRuntime.win-x86_64.*.nupkg"
    } 
    if ($RID -eq 'osx-x64') {
        $PackageIdPattern = "Imageflow.NativeRuntime.osx-x64.*.nupkg"
    }
    # Add other overrides if necessary, e.g., ubuntu-x86_64 maps to linux-x64
    # elseif ($RID -eq 'linux-x64') { ... }
    
    $NativePackagePath = Get-ChildItem -Path $PackOutputDirectory -Filter $PackageIdPattern | Select-Object -First 1 -ExpandProperty FullName
    
    if ($NativePackagePath -and (Test-Path $NativePackagePath)) {
        $InspectDir = Join-Path $WorkspaceRoot "temp_inspect_nupkg_single"
        if (Test-Path $InspectDir) { Remove-Item -Recurse -Force $InspectDir }
        New-Item -ItemType Directory -Path $InspectDir | Out-Null
        Write-Host "Inspecting package: $NativePackagePath"
        Write-Host "Extracting to: $InspectDir ..."
        try {
            Expand-Archive -Path $NativePackagePath -DestinationPath $InspectDir -Force -ErrorAction Stop
            Write-Host "Extraction successful. Contents:"
            Get-ChildItem -Path $InspectDir -Recurse | ForEach-Object { Write-Host $_.FullName }
            
            # Specific check for native file
            $ExpectedNativePath = "runtimes/$RID/native/"
            $NativeFileExists = Test-Path (Join-Path $InspectDir $ExpectedNativePath)
            Write-Host "Checking for native file existence in '$ExpectedNativePath': $NativeFileExists"
            if ($NativeFileExists){
                 Get-ChildItem (Join-Path $InspectDir $ExpectedNativePath) | ForEach-Object { Write-Host "  -> $($_.Name)" }
            }

        } catch {
            Write-Warning "Failed to extract or list package contents: $($_.Exception.Message)"
        }
        # Cleanup of inspect dir should happen in the main script's finally block or manually
    } else {
        Write-Warning "Could not find native runtime package in '$PackOutputDirectory' matching '$PackageIdPattern' to inspect."
    }
    Write-Host "--- DEBUG END (single-pack) ---" -ForegroundColor Magenta
    # --- END DEBUG ---

} catch {
    Write-Error "❌ Packing for RID $RID FAILED: $($_.Exception.Message)"
    exit 1
}


# --- 2. Build and Run EndToEnd Test Executable --- 
# Only run test if the runtime package was actually built
if (Test-Path $NativeRuntimeProject) {
    Write-Host "`n--- Building and Running EndToEnd Test App for RID ${RID} ---" -ForegroundColor Yellow
    try {
        Write-Host "Restoring & Building test project ($EndToEndTestProject) with local packages..."

        # Explicitly restore first with --force-evaluate
        dotnet restore $EndToEndTestProject -r $RID /p:RuntimePackageVersion=$PackageVersion /p:ImageflowNetVersion=$ImageflowNetVersion --force-evaluate -v diag

        if ($LASTEXITCODE -ne 0) {
            Write-Error "❌ Restoring $EndToEndTestProject FAILED."
            exit 1
        }

        Write-Host "Building test project ($EndToEndTestProject)..."
        # Build the console app, specifying the RID and using --no-restore
        dotnet build $EndToEndTestProject -c $Configuration -r $RID /p:RuntimePackageVersion=$PackageVersion /p:ImageflowNetVersion=$ImageflowNetVersion --no-restore

        if ($LASTEXITCODE -ne 0) {
            Write-Error "❌ Building $EndToEndTestProject FAILED."
            exit 1
        }

        # Construct path to the built executable
        $testExeDir = Join-Path $WorkspaceRoot "dotnet/nuget/test/bin/$Configuration/net8.0/$RID/publish"
        # Check if self-contained, if not, the path might be different
        $testExeName = (Get-Item (Join-Path $WorkspaceRoot "dotnet/nuget/test/Imageflow.EndToEnd.Test.csproj")).BaseName
        if ($IsWindows) { $testExeName += ".exe" }
        $testExePath = Join-Path $testExeDir $testExeName
        
        # Fallback path if not published self-contained (adjust if needed based on build output)
        if (-not (Test-Path $testExePath)){
             $testExeDir = Join-Path $WorkspaceRoot "dotnet/nuget/test/bin/$Configuration/net8.0/$RID"
             $testExePath = Join-Path $testExeDir $testExeName
        }

        if (-not (Test-Path $testExePath)) {
            Write-Error "❌ Test executable not found after build at expected path: $testExePath (or common variations)"
            exit 1
        }

        Write-Host "Running test executable: $testExePath ..."
        # Run the executable directly
        & $testExePath
        $exitCode = $LASTEXITCODE

        # --- DEBUG: Check .deps.json, drop .exe first ---  
        $testExeWithoutExe = $testExeName -replace '\.exe$', ''
        $depsJsonPath = Join-Path $testExeDir "$($testExeWithoutExe).deps.json"
        Write-Host "`n--- DEBUG: Contents of $depsJsonPath ---" -ForegroundColor Magenta
        if (Test-Path $depsJsonPath) {
            Get-Content $depsJsonPath | Write-Host
        } else {
            Write-Warning "File not found: $depsJsonPath"
        }
        Write-Host "--- DEBUG END ---" -ForegroundColor Magenta
        # --- END DEBUG ---

        if ($exitCode -ne 0) {
            Write-Error "❌ EndToEnd Test App FAILED for RID ${RID} with exit code $exitCode" # Used ${RID}
            exit 1
        }

        Write-Host "✅ EndToEnd Test App SUCCEEDED for RID ${RID}." -ForegroundColor Green # Used ${RID}

    } catch {
        # Reverted to original Write-Error with emoji and variable expansion
        Write-Error "❌ EndToEnd Test App Build/Run FAILED for RID ${RID}: $($_.Exception.Message)"
        exit 1
    }
} else {
     Write-Host "`n--- Skipping EndToEnd Test App Build/Run for RID $RID (No Runtime Package) ---" -ForegroundColor Yellow
}

Write-Host "`nScript single-pack-and-test.ps1 completed successfully for RID $RID." -ForegroundColor Cyan
Write-Host "`tNative Artifacts: $NativeArtifactBasePath" -ForegroundColor Cyan
Write-Host "`tPackages Output: $PackOutputDirectory" -ForegroundColor Cyan 
