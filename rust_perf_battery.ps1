<#
.SYNOPSIS
    Continuously runs `cargo clean` followed by `cargo build` in a loop.
    Logs comprehensive performance and battery statistics per iteration.

.DESCRIPTION
    - Runs a self-test before starting. If self-test fails, prints error and exits.
    - On start, prints initial line with date/time, battery, mode, etc.
    - For each iteration (called a "run"):
        * Executes cargo clean and cargo build
        * Measures times and increments counters
        * Sleeps a configured interval
        * Tracks stats per power-mode combination:
            - Total mode time
            - Total build/clean times
            - Build count
        * Calculates ratios:
            - Builds per % battery drop
            - Build time per % battery drop
            - Wall time per % battery drop
        * Prints a multi-line summary for each iteration:
            1) Overall line (date/time, run #, mode, battery, total times)
            2) Indented line with sleep info and last build output
            3) One indented line per mode with detailed stats and ratios
    - On command failure, prints the error output and stops.

    Time formats:
      - Display time: "yyyy-MM-dd HH:mm:sss" (with 's' appended at the end)
      - File names: "yyyy-MM-dd_HH-mm-ss"
      - Time spans: "HH:MM:SSs" for total times
      - Averages and ratios in seconds (with one decimal place if needed).

    "Other" time = total script runtime - (build+clean+sleep).

.NOTES
    Requires PowerShell 5.1+ and `cargo` in PATH.
    Assumes the current directory has a Cargo project (Cargo.toml).

.EXAMPLE
    .\rust_perf_battery.ps1
#>

param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Configuration
$PauseBetweenRuns = 10 # seconds to sleep between each iteration

# Formats
$logTimeFormat = "yyyy-MM-dd_HH-mm-ss"    # for file names
$displayTimeFormat = "yyyy-MM-dd HH:mm:ss" # for printed lines
[DateTime]$GlobalScriptStart = Get-Date
[string]$ScriptStartTime = (Get-Date -Format $logTimeFormat)
[string]$FullLogPath = Join-Path (Get-Location) ("rust_perf_battery_full_$ScriptStartTime.log")
[string]$SummaryLogPath = Join-Path (Get-Location) ("rust_perf_battery_summary_$ScriptStartTime.log")

# Track global stats
$CompileCount = 0
$StopLoop = $false
[TimeSpan]$TotalBuildTime = [TimeSpan]::Zero
[TimeSpan]$TotalCleanTime = [TimeSpan]::Zero
[TimeSpan]$TotalSleepTime = [TimeSpan]::Zero

# Mode stats: Dictionary<string,PSObject>
# Each value: { Mode, TotalModeTime, TotalBuildTime, TotalCleanTime, BuildCount }
$ModeStats = [System.Collections.Generic.Dictionary[string, object]]::new()

function Ensure-ModeStats($mode) {
    if (-not $ModeStats.ContainsKey($mode)) {
        $ModeStats[$mode] = [PSCustomObject]@{
            Mode = $mode
            TotalModeTime = [TimeSpan]::Zero
            TotalBuildTime = [TimeSpan]::Zero
            TotalCleanTime = [TimeSpan]::Zero
            BuildCount = 0
        }
    }
}

function Get-PowerSchemeName {
    $output = powercfg /getactivescheme 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($output)) {
        throw "Unable to retrieve power scheme name."
    }

    if ($output -match '\((?<SchemeName>[^)]+)\)') {
        return ($Matches['SchemeName'].Trim())
    } else {
        throw "No scheme name found in powercfg output."
    }
}

function Get-BatteryStatus {
    $batt = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue
    if (-not $batt) {
        return [PSCustomObject]@{
            HasBattery     = $false
            BatteryPercent = 100
            OnAC           = $true
            BatteryStatus  = "NoBattery"
        }
    }

    $statusCode = [int]$batt.BatteryStatus
    $percent = [int]$batt.EstimatedChargeRemaining
    $onAC = $true
    if ($statusCode -eq 1 -or $statusCode -eq 4 -or $statusCode -eq 5) {
        $onAC = $false
    }

    return [PSCustomObject]@{
        HasBattery     = $true
        BatteryPercent = $percent
        OnAC           = $onAC
        BatteryStatus  = $statusCode
    }
}

function Get-CurrentEnergyMode {
    $schemeName = Get-PowerSchemeName
    $battery = Get-BatteryStatus
    $powerSource = if ($battery.OnAC) { "ac" } else { "battery" }
    $modeName = ($schemeName -replace '\s+', '-').ToLower()
    return "$powerSource-$modeName"
}

function Format-ShortTime($ts) {
    # Format as HH:MM:SSs
    "{0:00}:{1:00}:{2:00}s" -f [int]$ts.TotalHours, $ts.Minutes, $ts.Seconds
}

function Self-Test {
    try {
        $mode = Get-CurrentEnergyMode
        # Just test we can add to mode stats without error
        Ensure-ModeStats($mode)
        $ModeStats[$mode].TotalModeTime += (New-TimeSpan -Seconds 5)
    } catch {
        Write-Host "Self-test failed: $($_.Exception.Message)"
        exit 1
    }
}

function Run-CargoClean {
    $start = Get-Date
    $output = & cargo clean 2>&1
    $exit = $LASTEXITCODE
    $elapsed = (Get-Date) - $start
    return [PSCustomObject]@{
        Output = $output
        Success = ($exit -eq 0)
        Elapsed = $elapsed
    }
}

function Run-CargoBuild {
    $start = Get-Date
    $output = & cargo build 2>&1
    $exit = $LASTEXITCODE
    $elapsed = (Get-Date) - $start
    return [PSCustomObject]@{
        Output = $output
        Success = ($exit -eq 0)
        Elapsed = $elapsed
    }
}

function Write-Logs {
    param(
        [string]$FullOutput,
        [string]$SummaryLine
    )
    Add-Content -Path $FullLogPath -Value $FullOutput
    Add-Content -Path $FullLogPath -Value $SummaryLine
    Add-Content -Path $SummaryLogPath -Value $SummaryLine
}

# Run self-test
Self-Test

# Clear mode stats since we tested adding time
$ModeStats.Clear()

# Now that functions are defined, we can read initial battery
$InitialBatteryPercent = (Get-BatteryStatus).BatteryPercent

# Print initial lines
$battery = Get-BatteryStatus
$initMode = Get-CurrentEnergyMode
$currentTimestamp = (Get-Date -Format $displayTimeFormat) + "s"

$initialLine = "$currentTimestamp battery=$($battery.BatteryPercent)% mode=$initMode"
Write-Host $initialLine

$logLine = "Logs: $(Split-Path $FullLogPath -Leaf), $(Split-Path $SummaryLogPath -Leaf)"
Write-Host $logLine

while (-not $StopLoop) {
    $iterationStart = Get-Date
    $CurrentEnergyMode = Get-CurrentEnergyMode

    # Clean
    $cleanResult = Run-CargoClean
    if (-not $cleanResult.Success) {
        Write-Host "cargo clean failed, output:"
        $cleanResult.Output | Write-Host
        $StopLoop = $true
        break
    }

    # Build
    $buildResult = Run-CargoBuild
    if (-not $buildResult.Success) {
        Write-Host "cargo build failed, output:"
        $buildResult.Output | Write-Host
        $StopLoop = $true
        break
    }

    $iterationElapsed = (Get-Date) - $iterationStart

    # Update global stats
    $TotalCleanTime += $cleanResult.Elapsed
    $TotalBuildTime += $buildResult.Elapsed

    # Update mode stats
    Ensure-ModeStats($CurrentEnergyMode)
    $ModeStats[$CurrentEnergyMode].TotalModeTime += $iterationElapsed
    $ModeStats[$CurrentEnergyMode].TotalBuildTime += $buildResult.Elapsed
    $ModeStats[$CurrentEnergyMode].TotalCleanTime += $cleanResult.Elapsed
    $ModeStats[$CurrentEnergyMode].BuildCount += 1

    $CompileCount++
    $batt = Get-BatteryStatus

    # Extract last build line safely
    $buildLineObj = $buildResult.Output | Select-Object -Last 1
    $lastBuildLine = if ($buildLineObj) { [string]$buildLineObj } else { "" }
    $lastBuildLine = $lastBuildLine.Trim()

    # Sleep between runs
    $sleepStart = Get-Date
    Start-Sleep -Seconds $PauseBetweenRuns
    $sleepElapsed = (Get-Date) - $sleepStart
    $TotalSleepTime += $sleepElapsed

    # Compute metrics
    $totalElapsed = (Get-Date) - $GlobalScriptStart
    $otherTime = $totalElapsed - ($TotalBuildTime + $TotalCleanTime + $TotalSleepTime)

    $totalBuildStr = Format-ShortTime $TotalBuildTime
    $totalCleanStr = Format-ShortTime $TotalCleanTime
    $totalSleepStr = Format-ShortTime $TotalSleepTime
    $otherTimeStr = Format-ShortTime $otherTime

    $batteryDrop = $InitialBatteryPercent - $batt.BatteryPercent
    $batteryDropFloat = [double]$batteryDrop
    $currentTimestamp = (Get-Date -Format $displayTimeFormat) + "s"
    $runCountStr = "{0:000}" -f $CompileCount

    # Overall line
    $overallLine = "$currentTimestamp, run $runCountStr, $CurrentEnergyMode, battery $($batt.BatteryPercent)%, total build $totalBuildStr, clean $totalCleanStr, sleep $totalSleepStr"
    Write-Host $overallLine

    # Second line
    $secondLine = "   Sleeping ${PauseBetweenRuns}s between builds. $lastBuildLine"
    Write-Host $secondLine

    # Mode lines with ratios
    foreach ($modeKey in $ModeStats.Keys) {
        $m = $ModeStats[$modeKey]
        # avg build/clean time
        $avgBuild = "N/A"
        $avgClean = "N/A"
        if ($m.BuildCount -gt 0) {
            $avgBuildSec = $m.TotalBuildTime.TotalSeconds / $m.BuildCount
            $avgBuild = ("{0:F1}s" -f $avgBuildSec)
            $avgCleanSec = $m.TotalCleanTime.TotalSeconds / $m.BuildCount
            $avgClean = ("{0:F1}s" -f $avgCleanSec)
        }

        $buildsPerDrop = "N/A"
        $buildTimePerDrop = "N/A"
        $wallPerDrop = "N/A"
        if ($batteryDropFloat -gt 0) {
            $buildsPerDropVal = $m.BuildCount / $batteryDropFloat
            $buildsPerDrop = ("{0:F2}" -f $buildsPerDropVal)

            $buildTimePerDropVal = $m.TotalBuildTime.TotalSeconds / $batteryDropFloat
            $buildTimePerDrop = ("{0:F0}s" -f $buildTimePerDropVal)

            $wallPerDropVal = $m.TotalModeTime.TotalSeconds / $batteryDropFloat
            $wallPerDrop = ("{0:F0}s" -f $wallPerDropVal)
        }

        $modeLine = "   On $($m.Mode) for $(Format-ShortTime $m.TotalModeTime), avg build $avgBuild, avg clean $avgClean, builds per % drop: $buildsPerDrop, build time per %: $buildTimePerDrop, wall per %: $wallPerDrop"
        Write-Host $modeLine
    }

    $fullOutput = "=== Iteration $CompileCount ===`r`n" +
                  "--- Cargo clean output ---`r`n" + ($cleanResult.Output -join "`r`n") + "`r`n" +
                  "--- Cargo build output ---`r`n" + ($buildResult.Output -join "`r`n") + "`r`n"

    # For logs, just write the overall line as summary
    Write-Logs -FullOutput $fullOutput -SummaryLine $overallLine
}

Write-Host "Script ended."
