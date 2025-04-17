# PowerShell Utility Functions for dotnet scripts

function Get-LatestImageflowNetVersion {
    Write-Host "--- Finding latest Imageflow.Net version (incl. prerelease) from nuget.org --- `n" -ForegroundColor Yellow

    $latestVersion = $null
    try {
        # Ensure NuGet provider is available
        if (-not (Get-PackageProvider -ListAvailable -Name NuGet)) {
            Write-Host "Installing NuGet package provider..."
            Install-PackageProvider -Name NuGet -MinimumVersion 2.8.5.201 -Force
        }
        # Find the latest version (incl. prereleases) by getting all and sorting
        Write-Host "Querying nuget.org for all Imageflow.Net versions..."
        # Get all versions, sort descending (latest first), take the top one
        $foundPackage = Find-Package Imageflow.Net -AllVersions -Source nuget.org | Sort-Object -Property Version -Descending | Select-Object -First 1
        
        if ($foundPackage) {
            $latestVersion = $foundPackage.Version.ToString()
            Write-Host "Found latest Imageflow.Net version: $latestVersion" -ForegroundColor Cyan
        } else {
            Write-Error "Could not find any version of Imageflow.Net on nuget.org."
            # Consider returning $null or throwing an exception based on desired behavior
            return $null 
        }
    } catch {
        Write-Error "Failed to query NuGet for Imageflow.Net version: $($_.Exception.Message)"
        # Consider returning $null or throwing an exception
        return $null 
    }
    return $latestVersion
}

# Export the function to make it available when dot-sourced
Export-ModuleMember -Function Get-LatestImageflowNetVersion 
