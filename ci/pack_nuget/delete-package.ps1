$packageIds = "Imageflow.NativeRuntime.ubuntu-arm64", "Imageflow.NativeTool.ubuntu-arm64"
$apiKey = $null
if ($null -eq $apiKey) {
    $apiKey = Read-Host "Enter your NuGet API key"
}

foreach($packageId in $packageIds) {
    $packageId = $packageId.ToLower();
    $json = Invoke-WebRequest -Uri "https://api.nuget.org/v3-flatcontainer/$packageId/index.json" | ConvertFrom-Json

    foreach($version in $json.versions)
    {
        Write-Host "Unlisting $packageId, Ver $version"
        dotnet nuget delete $packageId $version --source https://api.nuget.org/v3/index.json --non-interactive --api-key $apiKey
    }
}
