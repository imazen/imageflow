<#
    Usage: powershell.exe -ExecutionPolicy Bypass -File .\test_zip.ps1
#>

# Get script directory and parent directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PackDir = Split-Path -Parent $ScriptDir

$failed = $false

<#
    Reasoning: Step 1 - Set up our test environment.
    Goal: Create a temporary folder with test files and define test archive filenames.
#>
# Create a temporary directory for testing
$TestRoot = Join-Path $env:TEMP "zip-test-$(Get-Random)"
$TestInputDir = Join-Path $TestRoot "input"
$TestOutputDir = Join-Path $TestRoot "output"
New-Item -ItemType Directory -Path $TestInputDir -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Path $TestOutputDir -ErrorAction Stop | Out-Null



# Create sample files
$FileA = Join-Path $TestInputDir "fileA.txt"
$FileB = Join-Path $TestInputDir "fileB.log"
"Content A" | Set-Content $FileA -ErrorAction Stop
"Content B" | Set-Content $FileB -ErrorAction Stop

# Create a subdirectory, also containing a sample file
$SubDir = Join-Path $TestInputDir "subfolder"
New-Item -ItemType Directory -Path $SubDir -ErrorAction Stop | Out-Null
$FileC = Join-Path $SubDir "fileC.txt"
"Content C" | Set-Content $FileC -ErrorAction Stop

# Define archives to be created (with and without .zip extension)
$ArchiveForwardSlash = Join-Path $TestOutputDir "archive_forward.zip"
$ArchiveWildcard = Join-Path $TestOutputDir "archive_wildcard.zip"
$ArchiveNupkg = Join-Path $TestOutputDir "test_package.nupkg"

<#
    Reasoning: Step 2 - Test using forward slashes in paths.
    Goal: Verify that zip.ps1 correctly handles forward slashes and places files in root.
#>
Write-Host "`n--- Testing zip.ps1 with forward slashes ---"

Push-Location $TestInputDir
try {
    & "$PackDir\zip.ps1" -ArchiveFile $ArchiveForwardSlash -CompressionLevel NoCompression -Paths $FileA.Replace('\','/'),$FileB.Replace('\','/')
}
catch {
    Write-Error "❌ Test failed with forward slash paths: $_"
    $failed = $true
}
Pop-Location

Write-Host "Checking if archive was created and contains files in root..."
$ExpectedForwardSlashZip = $ArchiveForwardSlash
if (Test-Path "$ExpectedForwardSlashZip") {
    # Create temp extraction directory
    $ExtractDir = Join-Path $TestOutputDir "extract_forward"
    New-Item -ItemType Directory -Path $ExtractDir -ErrorAction Stop | Out-Null
    Expand-Archive -Path $ExpectedForwardSlashZip -DestinationPath $ExtractDir -ErrorAction Stop
    
    # Verify files are in root
    if ((Test-Path (Join-Path $ExtractDir "fileA.txt")) -and 
        (Test-Path (Join-Path $ExtractDir "fileB.log"))) {
        Write-Host "✅ SUCCESS: Archive with forward slashes created and files are in root"

    } else {
        Write-Error "❌ FAIL: Files not found in root of archive"
        Write-Host "Archive contents: $(Get-ChildItem -Path $ExtractDir -Recurse)"
        $failed = $true
    }
} else {
    Write-Error "❌ FAIL: Archive with forward slashes was not found."
    $failed = $true
}

<#
    Reasoning: Step 3 - Test using wildcard expansion.
    Goal: Verify that using '.' as path correctly expands to all files in directory.
#>
Write-Host "`n--- Testing zip.ps1 with wildcard expansion ---"

Push-Location $TestInputDir
try {
    & "$PackDir\zip.ps1" -ArchiveFile $ArchiveWildcard -CompressionLevel Fastest -Paths "."
}
catch {
    Write-Error "Test failed with wildcard expansion: $_"
    $failed = $true
}
Pop-Location

Write-Host "Checking if wildcard archive contains all files in root..."
if (Test-Path $ArchiveWildcard) {
    $ExtractWildcardDir = Join-Path $TestOutputDir "extract_wildcard"
    New-Item -ItemType Directory -Path $ExtractWildcardDir -ErrorAction Stop | Out-Null
    Expand-Archive -Path $ArchiveWildcard -DestinationPath $ExtractWildcardDir -ErrorAction Stop
    
    # Verify all files are present in root
    $AllFilesPresent = $true
    foreach ($file in @("fileA.txt", "fileB.log", "subfolder\fileC.txt")) {
        if (-not (Test-Path (Join-Path $ExtractWildcardDir $file))) {
            $AllFilesPresent = $false
            Write-Error "FAIL: File $file not found in expected location"
            $failed = $true
        }
    }
    if ($AllFilesPresent) {
        Write-Host "✅ SUCCESS: Wildcard expansion worked correctly"
    }
} else {
    Write-Error "❌ FAIL: Wildcard archive was not created"
    $failed = $true
}

<#
    Reasoning: Step 4 - Test .nupkg extension handling
    Goal: Verify that zip.ps1 correctly handles .nupkg files by creating a .zip archive
#>
Write-Host "`n--- Testing zip.ps1 with .nupkg extension ---"

Push-Location $TestInputDir
try {
    & "$PackDir\zip.ps1" -ArchiveFile $ArchiveNupkg -CompressionLevel Optimal -Paths $FileA,$FileB
}
catch {
    Write-Error "Test failed with .nupkg extension: $_"
    $failed = $true
}
Pop-Location

Write-Host "Checking if .nupkg archive was created and contains files..."
if (Test-Path $ArchiveNupkg) {
    $ExtractNupkgDir = Join-Path $TestOutputDir "extract_nupkg"
    New-Item -ItemType Directory -Path $ExtractNupkgDir -ErrorAction Stop | Out-Null
    
    # Try to extract the .nupkg file (it should be a valid zip file)
    try {
        # Rename the file to .zip before extracting
        $ZipArchive = $ArchiveNupkg -replace '\.nupkg$','.zip'
        $zipJustNameAndExt = [System.IO.Path]::GetFileName($ZipArchive)
        Rename-Item -Path $ArchiveNupkg -NewName $zipJustNameAndExt -ErrorAction Stop
        Expand-Archive -Path $ZipArchive -DestinationPath $ExtractNupkgDir -ErrorAction Stop

        # Verify files are present in root
        if ((Test-Path (Join-Path $ExtractNupkgDir "fileA.txt")) -and
            (Test-Path (Join-Path $ExtractNupkgDir "fileB.log"))) {
            Write-Host "✅ SUCCESS: .nupkg archive created and files are in root"
        } else {
            Write-Error "❌ FAIL: Files not found in root of .nupkg archive"
            $failed = $true
        }
    }
    catch {
        Write-Error "❌ FAIL: Could not extract .nupkg file as zip archive: $_"
        $failed = $true
    }
    finally {
        # Rename back to nupkg
         if (Test-Path $ZipArchive) {
            $nupkgJustNameAndExt = [System.IO.Path]::GetFileName($ArchiveNupkg)
            Rename-Item -Path $ZipArchive -NewName $nupkgJustNameAndExt -ErrorAction Stop
         }
    }
} else {
    Write-Error "❌ FAIL: .nupkg archive was not created"
    $failed = $true
}

<#
    Reasoning: Step 5 - Teardown and clean up test artifacts.
    Goal: Remove test files and directories after testing is complete.
#>
Write-Host "`n--- Cleaning up test artifacts ---"
try {
    Remove-Item -Path $TestRoot -Recurse -Force -ErrorAction Stop
    Write-Host "✅ Cleanup successful. Test completed."
}
catch {
    Write-Host "❌ Cleanup failed: $_"
    $failed = $true
}

if ($failed) {
  exit 1
} else {
  exit 0
} 
