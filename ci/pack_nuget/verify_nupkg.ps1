<#
.SYNOPSIS
    Verifies the structure and .nuspec content of a .nupkg file.
.DESCRIPTION
    - Takes a path to a .nupkg file
    - Extracts it to a temporary directory
    - Verifies the .nuspec file exists in the root and is valid XML
    - Optionally, compares the extracted .nuspec against a static gold file.
    - Cleans up temporary files
    - Returns 0 if valid, 1 if invalid
.EXAMPLE
    powershell.exe -File verify_nupkg.ps1 -NupkgPath path/to/package.nupkg
.EXAMPLE
    powershell.exe -File verify_nupkg.ps1 -NupkgPath path/to/package.nupkg -GoldNuspecPath tests/gold_nuspecs/tool.nuspec
#>

param(
    [Parameter(Mandatory=$true)]
    [string] $NupkgPath,

    # Optional: Path to the static gold .nuspec file for comparison
    [string] $GoldNuspecPath
)

function Test-NupkgStructure {
    param(
        [Parameter(Mandatory=$true)]
        [string] $NupkgPath,
        # Pass GoldNuspecPath down to the function
        [string] $GoldNuspecPath
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

    #Fix up /c/ to C:/
    $NupkgPath = $NupkgPath -replace '/c/', 'C:/'
    $NupkgPath = $NupkgPath -replace '\\c\\', 'C:/'

    # Create unique temp directory
    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) "nupkg-verify-$(Get-Random)"
    
    # Declare zipPath here so it's available in finally
    $zipPath = $NupkgPath -replace '\.nupkg$','.zip'
    $renamed = $false

    try {
        # Create temp directory
        New-Item -ItemType Directory -Path $tempDir | Out-Null
        Write-Host "Created temp directory: $tempDir"

        # Extract the package
        Write-Host "Extracting package to verify structure..."
        # Only rename if the file exists
        if (Test-Path $NupkgPath) {
            $zipJustNameAndExt = [System.IO.Path]::GetFileName($zipPath)
            Write-Host "Renaming $NupkgPath to $zipJustNameAndExt"
            Rename-Item -Path $NupkgPath -NewName $zipJustNameAndExt
            $renamed = $true
        }
        # Check if zip file exists before expanding
        if (Test-Path $zipPath) {
            Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force
        } else {
            Write-Error "Failed to find package file after potential rename: $zipPath"
            return $false
        }

        # Look for .nuspec file in root
        $nuspecFiles = @($zip.Entries | Where-Object { $_.Name -like '*.nuspec' })
        
        if ($nuspecFiles.Count -eq 0) {
            Write-Host "::error::No .nuspec file found in package root of $NupkgPath"
            Write-Error "No .nuspec file found in package root"
            return $false
        }
        
        if ($nuspecFiles.Count -gt 1) {
            Write-Host "::error::Multiple .nuspec files found in package root of $NupkgPath"
            Write-Error "Multiple .nuspec files found in package root"
            return $false
        }

        Write-Host "Found .nuspec file: $($nuspecFiles[0].Name)"

        # Load the actual nuspec content
        $nuspecPath = $nuspecFiles[0].FullName
        try {
            [xml]$actualNuspecXml = Get-Content -Path $nuspecPath -Raw
            Write-Host "Successfully parsed actual .nuspec as XML."
        } catch {
            Write-Host "::error::Failed to parse actual .nuspec file '$nuspecPath' as XML from package $NupkgPath. Content:"
            Get-Content -Path $nuspecPath -Raw | Write-Host
            Write-Error "Failed to parse actual .nuspec as XML: $_"
            return $false
        }

        # Perform Gold File Comparison if requested
        if (-not [string]::IsNullOrEmpty($GoldNuspecPath)) {
            Write-Host "Performing comparison against gold file: $GoldNuspecPath"
            if (-not (Test-Path $GoldNuspecPath)) {
                Write-Host "::error::Gold nuspec file not found: $GoldNuspecPath"
                Write-Error "Gold nuspec file not found: $GoldNuspecPath"
                return $false
            }

            # Read the static gold nuspec content
            $goldContent = Get-Content -Path $GoldNuspecPath -Raw

            # Load gold content as XML
            try {
                [xml]$goldNuspecXml = $goldContent
                Write-Host "Successfully parsed gold .nuspec '$GoldNuspecPath' as XML."
            } catch {
                Write-Host "::error::Failed to parse gold nuspec file '$GoldNuspecPath' as XML. Content:"
                $goldContent | Write-Host
                Write-Error "Failed to parse gold nuspec as XML: $_"
                return $false
            }

            # Compare the outer XML (structure and metadata)
            # Using OuterXml comparison, might be sensitive to formatting/whitespace
            # Consider more robust XML diff if needed
            if ($actualNuspecXml.OuterXml -ne $goldNuspecXml.OuterXml) {
                Write-Host "::error::Nuspec content does not match gold file! Package: $NupkgPath"
                Write-Host "--- ACTUAL --- ($nuspecPath)"
                $actualNuspecXml.OuterXml | Write-Host
                Write-Host "--- EXPECTED --- (from $GoldNuspecPath)"
                $goldNuspecXml.OuterXml | Write-Host
                Write-Host "--- DIFF ---"
                # Use Compare-Object for a basic diff
                Compare-Object -ReferenceObject ($actualNuspecXml.OuterXml -split '\r?\n') -DifferenceObject ($goldNuspecXml.OuterXml -split '\r?\n') | Format-Table -AutoSize | Out-String | Write-Host
                Write-Error "Nuspec content mismatch."
                return $false
            }
            Write-Host "Nuspec content matches gold file."
        }

        # Additional check: Verify <files> structure if targets were expected
        # The comparison above should cover this, but explicit check is good.
        if (-not [string]::IsNullOrEmpty($ExpectedTargetsFileName)) {
            $fileElements = @($actualNuspecXml.package.files.file)
            if ($null -eq $fileElements -or $fileElements.Count -ne 2) {
                Write-Host "::error::Expected 2 <file> elements for targets in nuspec, found $($fileElements.Count)"
                Write-Error "Incorrect number of <file> elements for targets."
                return $false
            }
            $target1 = "build\\net45\\$ExpectedTargetsFileName"
            $target2 = "buildTransitive\\net45\\$ExpectedTargetsFileName"
            if (($fileElements[0].target -ne $target1 -or $fileElements[1].target -ne $target2) -and `
                ($fileElements[0].target -ne $target2 -or $fileElements[1].target -ne $target1)) {
                    Write-Host "::error::Unexpected <file target=...> values for targets file."
                    $fileElements | Format-Table | Out-String | Write-Host
                    Write-Error "Incorrect target paths for targets files."
                    return $false
                }
            Write-Host "Verified <files> section for targets correctly."
        }

        # If we got here, all checks passed
        return $true
    }
    catch {
        Write-Host "::error::Failed to verify package $NupkgPath : $_"
        Write-Error "Failed to verify package: $_"
        return $false
    }
    finally {
        # Clean up temp directory
        if (Test-Path $tempDir) {
            Remove-Item -Path $tempDir -Recurse -Force
            Write-Host "Cleaned up temp directory"
        }
        # Rename file back if it was renamed
        if ($renamed -and (Test-Path $zipPath)) {
            $nupkgJustNameAndExt = [System.IO.Path]::GetFileName($NupkgPath)
            Write-Host "Renaming $zipPath back to $nupkgJustNameAndExt"
            Rename-Item -Path $zipPath -NewName $nupkgJustNameAndExt
        }
    }
}

# Run the verification and exit with appropriate code
# Pass the script-level GoldNuspecPath parameter to the function
if (Test-NupkgStructure -NupkgPath $NupkgPath -GoldNuspecPath $GoldNuspecPath) {
    Write-Host "Package structure verification succeeded"
    exit 0
} else {
    Write-Host "Package structure verification failed"
    exit 1
} 
