<#
.SYNOPSIS
    Continuously runs `cargo clean` followed by `cargo build` in a loop.
    Logs comprehensive performance and battery statistics per iteration.
    Prevents the system from sleeping or turning off the display during execution,
    and restores the previous system state on script exit.
    If the computer sleeps, it will cause a large time jump in iteration time; 
    we detect that and terminate.

.DESCRIPTION
    - Runs a self-test before starting. If self-test fails, prints error and exits.
    - On start, prevents the computer from sleeping or turning off the display by using the Win32 API SetThreadExecutionState.
    - On start, prints initial line with date/time, battery, mode, etc.
    - For each iteration:
        * Executes `cargo clean` then `cargo build`
        * Sleeps a configured interval between runs
        * Tracks performance stats, including battery usage
        * Logs results to full and summary log files
        * If any iteration takes unexpectedly long (e.g., due to sleep/resume),
          we detect a time jump and exit. By default, "unexpectedly long" is >3x a baseline of 5 minutes.
    - On command failure, prints the error output and stops.
    - On AC power, we do not show battery drop-related stats since they don't make sense.
    - On script end (or if killed), attempts to restore system sleep/display-off behavior.

    Time formats:
      - Display time: "yyyy-MM-dd HH:mm:sss" (with 's' appended)
      - File names: "yyyy-MM-dd_HH-mm-ss"
      - Time spans: "HH:MM:SSs" for total times

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
$BaselineIterationTime = [TimeSpan]::FromMinutes(5)
$MaxAllowedIterationTime = New-TimeSpan -Minutes ($BaselineIterationTime.TotalMinutes * 3) # 3x baseline

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
            HasBattery         = $false
            BatteryPercent     = 100
            OnAC               = $true
            BatteryStatus      = "NoBattery"
            FullChargeCapacity = $null
        }
    }

    $statusCode = [int]$batt.BatteryStatus
    $percent = [int]$batt.EstimatedChargeRemaining
    $onAC = $true
    if ($statusCode -eq 1 -or $statusCode -eq 4 -or $statusCode -eq 5) {
        $onAC = $false
    }

    $fcc = $batt.FullChargeCapacity
    if (-not $fcc) {
        $fcc = $null
    }

    return [PSCustomObject]@{
        HasBattery         = $true
        BatteryPercent     = $percent
        OnAC               = $onAC
        BatteryStatus      = $statusCode
        FullChargeCapacity = $fcc
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
    "{0:00}:{1:00}:{2:00}s" -f [int]$ts.TotalHours, $ts.Minutes, $ts.Seconds
}

function Format-HoursMinutes($ts) {
    $totalMinutes = [int][Math]::Floor($ts.TotalMinutes)
    $h = [int]($totalMinutes / 60)
    $m = $totalMinutes % 60
    if ($h -gt 0 -and $m -gt 0) {
        return "{0}h{1}m" -f $h, $m
    } elseif ($h -gt 0) {
        return "{0}h" -f $h
    } else {
        return "{0}m" -f $m
    }
}

function Self-Test {
    try {
        $mode = Get-CurrentEnergyMode
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

# Prevent sleep/display off
try {
    [void][SleepPreventer.Power]
} catch {
    Add-Type -Namespace SleepPreventer -Name Power -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("kernel32.dll", CharSet=System.Runtime.InteropServices.CharSet.Auto,SetLastError=true)]
public static extern System.UInt32 SetThreadExecutionState(System.UInt64 esFlags);
"@
}

[uint64]$ES_CONTINUOUS = 2147483648
[uint64]$ES_SYSTEM_REQUIRED = 1
[uint64]$ES_DISPLAY_REQUIRED = 2

[SleepPreventer.Power]::SetThreadExecutionState($ES_CONTINUOUS -bor $ES_SYSTEM_REQUIRED -bor $ES_DISPLAY_REQUIRED) | Out-Null

Self-Test
$ModeStats.Clear()

$initBattery = Get-BatteryStatus
$InitialBatteryPercent = $initBattery.BatteryPercent
$FullChargeCapacity = $initBattery.FullChargeCapacity
$InitialEnergy_mWh = $null
if ($FullChargeCapacity -and $FullChargeCapacity -gt 0) {
    $InitialEnergy_mWh = $FullChargeCapacity * ($InitialBatteryPercent / 100.0)
}

$currentTimestamp = (Get-Date -Format $displayTimeFormat) + "s"
$initMode = Get-CurrentEnergyMode
$initialLine = "$currentTimestamp battery=$($initBattery.BatteryPercent)% mode=$initMode"
Write-Host $initialLine

$logLine = "Logs: $(Split-Path $FullLogPath -Leaf), $(Split-Path $SummaryLogPath -Leaf)"
Write-Host $logLine

try {
    while (-not $StopLoop) {
        $iterationStart = Get-Date
        $CurrentEnergyMode = Get-CurrentEnergyMode

        $cleanResult = Run-CargoClean
        if (-not $cleanResult.Success) {
            Write-Host "cargo clean failed, output:"
            $cleanResult.Output | Write-Host
            $StopLoop = $true
            break
        }

        $buildResult = Run-CargoBuild
        if (-not $buildResult.Success) {
            Write-Host "cargo build failed, output:"
            $buildResult.Output | Write-Host
            $StopLoop = $true
            break
        }

        $sleepStart = Get-Date
        Start-Sleep -Seconds $PauseBetweenRuns
        $sleepElapsed = (Get-Date) - $sleepStart

        $iterationElapsed = (Get-Date) - $iterationStart

        # Check for time jump
        if ($iterationElapsed -gt $MaxAllowedIterationTime) {
            Write-Host "Detected time jump (iteration took $($iterationElapsed.ToString())) - possibly slept. Exiting."
            break
        }

        # Update stats
        $TotalSleepTime += $sleepElapsed
        $TotalCleanTime += $cleanResult.Elapsed
        $TotalBuildTime += $buildResult.Elapsed

        Ensure-ModeStats($CurrentEnergyMode)
        $ModeStats[$CurrentEnergyMode].TotalModeTime += $iterationElapsed
        $ModeStats[$CurrentEnergyMode].TotalBuildTime += $buildResult.Elapsed
        $ModeStats[$CurrentEnergyMode].TotalCleanTime += $cleanResult.Elapsed
        $ModeStats[$CurrentEnergyMode].BuildCount += 1

        $CompileCount++
        $batt = Get-BatteryStatus

        $buildLineObj = $buildResult.Output | Select-Object -Last 1
        $lastBuildLine = if ($buildLineObj) { [string]$buildLineObj } else { "" }
        $lastBuildLine = $lastBuildLine.Trim()

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

        $UsedEnergy_mWh = $null
        $mWh_per_build = "N/A"
        if ($FullChargeCapacity -and $FullChargeCapacity -gt 0 -and $InitialEnergy_mWh -ne $null) {
            $CurrentEnergy_mWh = $FullChargeCapacity * ($batt.BatteryPercent / 100.0)
            $UsedEnergy_mWh = $InitialEnergy_mWh - $CurrentEnergy_mWh
            if ($CompileCount -gt 0) {
                $mWh_per_build_val = $UsedEnergy_mWh / $CompileCount
                $mWh_per_build = ("{0:F2} mWh/build" -f $mWh_per_build_val)
            } else {
                $mWh_per_build = "N/A"
            }
        }

        $energyLine = ""
        if ($UsedEnergy_mWh -ne $null) {
            $energyLine = ", used {0:F2}mWh, {1}" -f $UsedEnergy_mWh, $mWh_per_build
        }

        $overallLine = "$currentTimestamp, run $runCountStr, $CurrentEnergyMode, battery $($batt.BatteryPercent)%, total build $totalBuildStr, clean $totalCleanStr, sleep $totalSleepStr$energyLine"
        Write-Host $overallLine

        # Second line
        $secondLine = "   Sleeping ${PauseBetweenRuns}s between builds. $lastBuildLine"
        Write-Host $secondLine

        # Mode lines
        $OnAC = $batt.OnAC
        foreach ($modeKey in $ModeStats.Keys) {
            $m = $ModeStats[$modeKey]
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
            $timeFor80Drop = "N/A"

            # Only show battery-drop related stats if not on AC
            if (-not $OnAC -and $batteryDropFloat -gt 0) {
                $buildsPerDropVal = $m.BuildCount / $batteryDropFloat
                $buildsPerDrop = ("{0:F2}" -f $buildsPerDropVal)

                $buildTimePerDropVal = $m.TotalBuildTime.TotalSeconds / $batteryDropFloat
                $buildTimePerDrop = ("{0:F0}s" -f $buildTimePerDropVal)

                $wallPerDropVal = $m.TotalModeTime.TotalSeconds / $batteryDropFloat
                $wallPerDrop = ("{0:F0}s" -f $wallPerDropVal)

                $wallFor80PctSec = $m.TotalModeTime.TotalSeconds / $batteryDropFloat * 80
                $wallFor80Pct = [TimeSpan]::FromSeconds($wallFor80PctSec)
                $timeFor80Drop = Format-HoursMinutes $wallFor80Pct
            }

            $modeLine = "   On $($m.Mode) for $(Format-ShortTime $m.TotalModeTime), avg build $avgBuild, avg clean $avgClean"
            if (-not $OnAC -and $batteryDropFloat -gt 0) {
                $modeLine += ", est. builds per % drop: $buildsPerDrop, build time per %: $buildTimePerDrop, wall per %: $wallPerDrop, 80% drop wall: $timeFor80Drop"
            }
            Write-Host $modeLine
        }

        $fullOutput = "=== Iteration $CompileCount ===`r`n" +
                      "--- Cargo clean output ---`r`n" + ($cleanResult.Output -join "`r`n") + "`r`n" +
                      "--- Cargo build output ---`r`n" + ($buildResult.Output -join "`r`n") + "`r`n"

        Write-Logs -FullOutput $fullOutput -SummaryLine $overallLine
    }
} finally {
    [SleepPreventer.Power]::SetThreadExecutionState($ES_CONTINUOUS) | Out-Null
}

Write-Host "Script ended."
