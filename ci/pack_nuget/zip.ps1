<#
.SYNOPSIS
    A simple wrapper around Compress-Archive to behave like 'zip' with enhanced error handling and file extension management.
.DESCRIPTION
    - Takes the first argument as the destination (zip) filename.
    - Takes all other arguments as files or directories to include.
    - Converts forward slashes to backslashes for Windows compatibility.
    - Appends .zip to the archive filename if not present and renames it back after compression.
    - Uses -Update to add to existing archives if they already exist.
    - When "." is specified as a path, expands to all files in current directory.
.EXAMPLE
    powershell.exe -File zip.ps1 archive.zip file1.txt folder/subfolder/file2.log
#>

param(
    [Parameter(Mandatory=$true, Position=0)]
    [string] $ArchiveFile,

    [Parameter(Mandatory=$true, Position=1, ValueFromRemainingArguments)]
    [string[]] $Paths
)

# Slashes don't matter, but /c/ needs to be C:/
$ArchiveFile = $ArchiveFile -replace '/c/', 'C:/'
$Paths = $Paths | ForEach-Object { $_ -replace '/c/', 'C:/' }
$ArchiveFile = $ArchiveFile -replace '\c\', 'C:/'
$Paths = $Paths | ForEach-Object { $_ -replace '\c\', 'C:\' }

$ZipAdded = $false

# Reasoning: Ensure the archive file has a .zip extension to comply with Compress-Archive requirements.
# Goal: Append .zip if the provided archive filename does not already end with .zip
if (-not $ArchiveFile.EndsWith('.zip', [System.StringComparison]::InvariantCultureIgnoreCase)) {
    $ArchiveFile += '.zip'
    $ZipAdded = $true
}

# Convert forward slashes and expand wildcards
$ExpandedPaths = @()
foreach ($path in $Paths) {
    $path = $path -replace '/', '\'
    if ($path -eq ".") {
        # When "." is specified, add all items in current directory
        $ExpandedPaths += Get-ChildItem -Path "." | ForEach-Object { $_.FullName }
    } else {
        $ExpandedPaths += $path
    }
}

Write-Host "Compressing the following items: $($ExpandedPaths -join ', ')"

try {
    if (Test-Path $ArchiveFile) {
        Write-Host "Archive '$ArchiveFile' exists. Adding new files..."
        Compress-Archive -Path $ExpandedPaths -DestinationPath $ArchiveFile -Update -ErrorAction Stop
    }
    else {
        Write-Host "Creating new archive '$ArchiveFile'..."
        Compress-Archive -Path $ExpandedPaths -DestinationPath $ArchiveFile -ErrorAction Stop
    }

    # Reasoning: Rename the archive back to the original filename if it was modified.
    # Goal: Maintain the user's intended archive filename without the .zip extension in the final output.
    if ($ZipAdded) {
        # Get just the filename from the archive file
        $FinalArchiveFileName = (Get-Item -Path $ArchiveFile).Name -replace '\.zip$', ''
        Write-Host "Renaming '$ArchiveFile' back to '$FinalArchiveFileName'..."
        Rename-Item -Path $ArchiveFile -NewName $FinalArchiveFileName -ErrorAction Stop
    }

    # Reasoning: Indicate successful completion of the compression and renaming process.
    # Goal: Provide user feedback upon successful archiving and renaming.
    Write-Host "Compression and renaming completed successfully."
}
catch {
    # Reasoning: Handle any errors that occur during the compression or renaming process.
    # Goal: Inform the user of the failure and exit with a non-zero code.
    Write-Error "Operation failed: $_"
    exit 1
}
