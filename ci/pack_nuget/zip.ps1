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

    [Parameter(Mandatory=$false, Position=1)]
    [string] $CompressionLevel = "Default",

    [Parameter(Mandatory=$true, Position=2, ValueFromRemainingArguments)]
    [string[]] $Paths
)

# if compression level is default, set to env var ZIP_PS1_COMPRESSION_LEVEL, falling back to "Optimal" if not set
if ($CompressionLevel -eq "Default") {
    $CompressionLevel = $env:ZIP_PS1_COMPRESSION_LEVEL
    if (-not $CompressionLevel) {
        $CompressionLevel = "Optimal"
    }else{
        Write-Host "Using compression level from ZIP_PS1_COMPRESSION_LEVEL environment variable: $CompressionLevel"
    }
    if ($CompressionLevel -ne "Optimal" -and $CompressionLevel -ne "Fastest" -and $CompressionLevel -ne "NoCompression") {
        Write-Error "Invalid compression level: $CompressionLevel, setting to Optimal"
        $CompressionLevel = "Optimal"
    }
}



# Slashes don't matter, but /c/ needs to be C:/
$ArchiveFile = $ArchiveFile -replace '/c/', 'C:/'
$Paths = $Paths | ForEach-Object { $_ -replace '/c/', 'C:/' }
$ArchiveFile = $ArchiveFile -replace '\c\', 'C:/'
$Paths = $Paths | ForEach-Object { $_ -replace '\c\', 'C:\' }
$ArchiveFile = $ArchiveFile -replace '\/', '\'

$ZipAdded = $false

$OriginalArchiveFileName = [System.IO.Path]::GetFileName($ArchiveFile)

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
        $ExpandedPaths += "*"# Get-ChildItem -Path "." | ForEach-Object { $_.FullName }
    } else {
        # TODO: Verify that the path exists?
        $ExpandedPaths += $path
    }
}
$PathsCommaSeparated = $ExpandedPaths -join ', '

Write-Host "Compressing ($CompressionLevel) the following items: $PathsCommaSeparated"

try {
    $compress = @{
        Path = $ExpandedPaths
        DestinationPath = $ArchiveFile
        CompressionLevel = $CompressionLevel
    }
    Compress-Archive @compress
    if (Test-Path $ArchiveFile) {
        Write-Host "Appending files to existing archive '$ArchiveFile'..."
        $compress['Update'] = $true
        Compress-Archive @compress -ErrorAction Stop
    }
    else {
        Write-Host "Creating archive '$ArchiveFile'..."
        Compress-Archive @compress -ErrorAction Stop
    }
    Write-Host "Compress-Archive completed."
    
    # Verify the file was created
    if (-not (Test-Path $ArchiveFile)) {
        Write-Error "Archive file '$ArchiveFile' was not created"
        exit 1
    }

    # Reasoning: Rename the archive back to the original filename if it was modified.
    # Goal: Maintain the user's intended archive filename without the .zip extension in the final output.
    if ($ZipAdded) {
        Write-Host "Renaming '$ArchiveFile' back to '$OriginalArchiveFileName'..."
        Rename-Item -Path $ArchiveFile -NewName $OriginalArchiveFileName -ErrorAction Stop
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
