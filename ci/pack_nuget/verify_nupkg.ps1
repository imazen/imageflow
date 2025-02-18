<#
.SYNOPSIS
    Verifies that a .nupkg file has a .nuspec file in its root directory.
.DESCRIPTION
    - Takes a path to a .nupkg file
    - Extracts it to a temporary directory
    - Verifies the .nuspec file exists in the root
    - Cleans up temporary files
    - Returns 0 if valid, 1 if invalid
.EXAMPLE
    powershell.exe -File verify_nupkg.ps1 path/to/package.nupkg
#>

param(
    [Parameter(Mandatory=$true)]
    [string] $NupkgPath
)

function Test-NupkgStructure {
    param(
        [Parameter(Mandatory=$true)]
        [string] $NupkgPath
    )

    # Validate input file exists and has .nupkg extension
    if (-not (Test-Path $NupkgPath)) {
        Write-Error "Package file not found: $NupkgPath"
        return $false
    }
    
    if (-not $NupkgPath.EndsWith('.nupkg', [StringComparison]::OrdinalIgnoreCase)) {
        Write-Error "File must have .nupkg extension: $NupkgPath"
        return $false
    }

    # Create unique temp directory
    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) "nupkg-verify-$(Get-Random)"
    
    try {
        # Create temp directory
        New-Item -ItemType Directory -Path $tempDir | Out-Null
        Write-Host "Created temp directory: $tempDir"

        # Extract the package
        Write-Host "Extracting package to verify structure..."
        Expand-Archive -Path $NupkgPath -DestinationPath $tempDir -Force

        # Look for .nuspec file in root
        $nuspecFiles = Get-ChildItem -Path $tempDir -Filter "*.nuspec"
        
        if ($nuspecFiles.Count -eq 0) {
            Write-Error "No .nuspec file found in package root"
            return $false
        }
        
        if ($nuspecFiles.Count -gt 1) {
            Write-Error "Multiple .nuspec files found in package root"
            return $false
        }

        Write-Host "Found .nuspec file: $($nuspecFiles[0].Name)"
        return $true
    }
    catch {
        Write-Error "Failed to verify package: $_"
        return $false
    }
    finally {
        # Clean up temp directory
        if (Test-Path $tempDir) {
            Remove-Item -Path $tempDir -Recurse -Force
            Write-Host "Cleaned up temp directory"
        }
    }
}

# Run the verification and exit with appropriate code
if (Test-NupkgStructure $NupkgPath) {
    Write-Host "Package structure verification succeeded"
    exit 0
} else {
    Write-Host "Package structure verification failed"
    exit 1
} 
