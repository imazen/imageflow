<#
.SYNOPSIS
    A simple wrapper around Compress-Archive to behave like 'zip'.
.DESCRIPTION
    - Takes the first argument as the destination (zip) filename.
    - Takes all other arguments as files or directories to include.
    - Converts forward slashes to backslashes for Windows compatibility.
    - Uses -Update to add to existing archives if they already exist.
.EXAMPLE
    powershell.exe -File zip.ps1 archive.zip file1.txt folder/subfolder/file2.log
#>

param(
    [Parameter(Mandatory=$true, Position=0)]
    [string] $ArchiveFile,

    [Parameter(Mandatory=$true, Position=1, ValueFromRemainingArguments)]
    [string[]] $Paths
)

# Convert forward slashes to backslashes in the archive name
$ArchiveFile = $ArchiveFile -replace '/', '\'

# Convert forward slashes to backslashes in all paths
$Paths = $Paths | ForEach-Object { $_ -replace '/', '\' }

# If the archive file does not exist, we can just do a normal compress.
# If it does exist, we'll use -Update to add files to it, if your PowerShell supports it.

if (Test-Path $ArchiveFile) {
    Write-Host "Archive '$ArchiveFile' exists. Adding new files..."
    Compress-Archive -Path $Paths -DestinationPath $ArchiveFile -Update
}
else {
    Write-Host "Creating new archive '$ArchiveFile' with specified files/directories..."
    Compress-Archive -Path $Paths -DestinationPath $ArchiveFile
}

Write-Host "Done."
