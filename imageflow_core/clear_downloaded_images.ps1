# Get the script's directory (assumed to be imageflow_core)
$scriptDir = $PSScriptRoot

# Set the source and destination directories relative to the script location
$sourceDir = Join-Path $scriptDir "tests" "visuals"
$backupDir = Join-Path $sourceDir "_backup"

# Create the backup directory if it doesn't exist
if (-not (Test-Path $backupDir)) {
    New-Item -ItemType Directory -Path $backupDir | Out-Null
    Write-Host "Created backup directory: $backupDir"
}

# Get all PNG, JPG, and WebP files in the source directory
$files = Get-ChildItem -Path $sourceDir -File -Include "*.png", "*.jpg", "*.jpeg", "*.webp"

foreach ($file in $files) {
    $destPath = Join-Path $backupDir $file.Name
    
    # Check if the file already exists in the backup directory
    if (Test-Path $destPath) {
        Write-Host "Skipping $($file.Name) - already exists in backup directory"
    } else {
        # Move the file to the backup directory
        Move-Item -Path $file.FullName -Destination $destPath
        Write-Host "Moved $($file.Name) to backup directory"
    }
}

Write-Host "Operation completed. Check $backupDir for moved files."
Write-Host "Source directory: $sourceDir"
