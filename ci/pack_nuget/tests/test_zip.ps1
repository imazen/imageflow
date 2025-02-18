<#
    Usage: powershell.exe -ExecutionPolicy Bypass -File .\test_zip.ps1
#>

# Get script directory and parent directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PackDir = Split-Path -Parent $ScriptDir

<#
    Reasoning: Step 1 - Set up our test environment.
    Goal: Create a temporary folder with test files and define test archive filenames.
#>
# Create a temporary directory for testing
$TestDir = Join-Path $env:TEMP "zip-test-$(Get-Random)"
New-Item -ItemType Directory -Path $TestDir | Out-Null

# Create sample files
$FileA = Join-Path $TestDir "fileA.txt"
$FileB = Join-Path $TestDir "fileB.log"
"Content A" | Set-Content $FileA
"Content B" | Set-Content $FileB

# Create a subdirectory, also containing a sample file
$SubDir = Join-Path $TestDir "subfolder"
New-Item -ItemType Directory -Path $SubDir | Out-Null
$FileC = Join-Path $SubDir "fileC.txt"
"Content C" | Set-Content $FileC

# Define archives to be created (with and without .zip extension)
$ArchiveForwardSlash = Join-Path $TestDir "archive_forward.huh"
$ArchiveBackwardSlash = Join-Path $TestDir "archive_backward.zip"
$ArchiveWildcard = Join-Path $TestDir "archive_wildcard.zip"
$ArchiveNupkg = Join-Path $TestDir "test_package.nupkg"

<#
    Reasoning: Step 2 - Test using forward slashes in paths.
    Goal: Verify that zip.ps1 correctly handles forward slashes and places files in root.
#>
Write-Host "`n--- Testing zip.ps1 with forward slashes ---"

Push-Location $TestDir
try {
    & "$PackDir\zip.ps1" $ArchiveForwardSlash $FileA.Replace('\','/') $FileB.Replace('\','/') 
}
catch {
    Write-Error "Test failed with forward slash paths: $_"
}
Pop-Location

Write-Host "Checking if archive was created and contains files in root..."
$ExpectedForwardSlashZip = $ArchiveForwardSlash
if (Test-Path "$ExpectedForwardSlashZip") {
    # Create temp extraction directory
    $ExtractDir = Join-Path $TestDir "extract_forward"
    New-Item -ItemType Directory -Path $ExtractDir | Out-Null
    Expand-Archive -Path $ExpectedForwardSlashZip -DestinationPath $ExtractDir
    
    # Verify files are in root
    if ((Test-Path (Join-Path $ExtractDir "fileA.txt")) -and 
        (Test-Path (Join-Path $ExtractDir "fileB.log"))) {
        Write-Host "SUCCESS: Archive with forward slashes created and files are in root"
    } else {
        Write-Error "FAIL: Files not found in root of archive"
        Write-Host "Archive contents: $(Get-ChildItem -Path $ExtractDir)"
    }
} else {
    Write-Error "FAIL: Archive with forward slashes was not found."
}

<#
    Reasoning: Step 3 - Test using wildcard expansion.
    Goal: Verify that using '.' as path correctly expands to all files in directory.
#>
Write-Host "`n--- Testing zip.ps1 with wildcard expansion ---"

Push-Location $TestDir
try {
    & "$PackDir\zip.ps1" $ArchiveWildcard "."
}
catch {
    Write-Error "Test failed with wildcard expansion: $_"
}
Pop-Location

Write-Host "Checking if wildcard archive contains all files in root..."
if (Test-Path $ArchiveWildcard) {
    $ExtractWildcardDir = Join-Path $TestDir "extract_wildcard"
    New-Item -ItemType Directory -Path $ExtractWildcardDir | Out-Null
    Expand-Archive -Path $ArchiveWildcard -DestinationPath $ExtractWildcardDir
    
    # Verify all files are present in root
    $AllFilesPresent = $true
    foreach ($file in @("fileA.txt", "fileB.log", "subfolder\fileC.txt")) {
        if (-not (Test-Path (Join-Path $ExtractWildcardDir $file))) {
            $AllFilesPresent = $false
            Write-Error "FAIL: File $file not found in expected location"
        }
    }
    if ($AllFilesPresent) {
        Write-Host "SUCCESS: Wildcard expansion worked correctly"
    }
} else {
    Write-Error "FAIL: Wildcard archive was not created"
}

<#
    Reasoning: Step 4 - Test .nupkg extension handling
    Goal: Verify that zip.ps1 correctly handles .nupkg files by creating a .zip archive
#>
Write-Host "`n--- Testing zip.ps1 with .nupkg extension ---"

Push-Location $TestDir
try {
    & "$PackDir\zip.ps1" $ArchiveNupkg $FileA $FileB
}
catch {
    Write-Error "Test failed with .nupkg extension: $_"
}
Pop-Location

Write-Host "Checking if .nupkg archive was created and contains files..."
if (Test-Path $ArchiveNupkg) {
    $ExtractNupkgDir = Join-Path $TestDir "extract_nupkg"
    New-Item -ItemType Directory -Path $ExtractNupkgDir | Out-Null
    
    # Try to extract the .nupkg file (it should be a valid zip file)
    try {
        Expand-Archive -Path $ArchiveNupkg -DestinationPath $ExtractNupkgDir
        
        # Verify files are present in root
        if ((Test-Path (Join-Path $ExtractNupkgDir "fileA.txt")) -and 
            (Test-Path (Join-Path $ExtractNupkgDir "fileB.log"))) {
            Write-Host "SUCCESS: .nupkg archive created and files are in root"
        } else {
            Write-Error "FAIL: Files not found in root of .nupkg archive"
        }
    }
    catch {
        Write-Error "FAIL: Could not extract .nupkg file as zip archive: $_"
    }
} else {
    Write-Error "FAIL: .nupkg archive was not created"
}

<#
    Reasoning: Step 5 - Teardown and clean up test artifacts.
    Goal: Remove test files and directories after testing is complete.
#>
Write-Host "`n--- Cleaning up test artifacts ---"
try {
    Remove-Item -Path $TestDir -Recurse -Force
    Write-Host "Cleanup successful. Test completed."
}
catch {
    Write-Host "Cleanup failed: $_"
} 
