<#
.SYNOPSIS
    Continuously runs `cargo clean` followed by `cargo build` in a loop.
    Logs comprehensive performance and battery statistics per iteration.
    Prevents the system from sleeping or turning off the display during execution,
    and restores the previous system state on script exit.
    Detects large time jumps (sleep/resume) and exits if found.
    If on AC power, skips battery-drop-based stats.
    If battery is low and presumably on battery saver (below ~30%), 
    appends "-saving" to the mode name.
    Uses 70% drop wall time estimate instead of 80%.
    The summary log file should exactly match what is printed to the terminal.

.DESCRIPTION
    - Runs a self-test before starting. If self-test fails, prints error and exits.
    - On start, prevents the computer from sleeping or turning off the display by using Win32 API SetThreadExecutionState.
      Uses 64-bit values for ES_CONTINUOUS etc.
    - On start, prints initial line with date/time, battery, mode, etc.
    - For each iteration:
        * Executes cargo clean then cargo build.
        * Sleeps a configured interval between runs.
        * Tracks performance stats, including battery usage (if available).
        * If on AC, does not show battery-drop-based statistics.
        * If battery below 30% and on battery, mode is "battery-<plan>-saving".
        * Calculates est. builds per % drop, build time per %, wall time per %,
          and now uses 70% drop wall time estimate per mode.
        * If iteration takes too long (>3x baseline time), assumes sleep/resume and exits.
    - On command failure, prints the error output and stops.
    - On script end (or if killed), attempts to restore system state.
    - The summary log matches exactly what's printed to terminal line-by-line.
    - The full log includes all summary lines plus cargo outputs.

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
    # If on battery and below 30%, assume "saving"
    $battery = Get-BatteryStatus
    $schemeName = Get-PowerSchemeName
    $powerSource = if ($battery.OnAC) { "ac" } else { "battery" }
    $modeName = ($schemeName -replace '\s+', '-').ToLower()

    if (-not $battery.OnAC -and $battery.BatteryPercent -le 30) {
        return "$powerSource-$modeName-saving"
    } else {
        return "$powerSource-$modeName"
    }
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

# We want summary log to exactly match what we print.
# We'll write a helper function for summary lines:
function Write-And-LogSummary {
    param([string]$Line)
    Write-Host $Line
    Add-Content $SummaryLogPath $Line
    Add-Content $FullLogPath $Line
}

# For cargo output, only goes to full logs (no printing):
function Write-FullOnly {
    param([string]$FullOutput)
    Add-Content $FullLogPath $FullOutput
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
Write-And-LogSummary $initialLine

$logLine = "Logs: $(Split-Path $FullLogPath -Leaf), $(Split-Path $SummaryLogPath -Leaf)"
Write-And-LogSummary $logLine

try {
    while (-not $StopLoop) {
        $iterationStart = Get-Date
        $CurrentEnergyMode = Get-CurrentEnergyMode

        $cleanResult = Run-CargoClean
        if (-not $cleanResult.Success) {
            Write-And-LogSummary "cargo clean failed, output:"
            foreach ($line in $cleanResult.Output) {
                Write-And-LogSummary $line
            }
            $StopLoop = $true
            break
        }

        $buildResult = Run-CargoBuild
        if (-not $buildResult.Success) {
            Write-And-LogSummary "cargo build failed, output:"
            foreach ($line in $buildResult.Output) {
                Write-And-LogSummary $line
            }
            $StopLoop = $true
            break
        }

        $sleepStart = Get-Date
        Start-Sleep -Seconds $PauseBetweenRuns
        $sleepElapsed = (Get-Date) - $sleepStart

        $iterationElapsed = (Get-Date) - $iterationStart

        if ($iterationElapsed -gt $MaxAllowedIterationTime) {
            Write-And-LogSummary "Detected time jump (iteration took $($iterationElapsed.ToString())) - possibly slept. Exiting."
            break
        }

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
            else {
                $mWh_per_build = "N/A"
            }
        }

        $energyLine = ""
        if ($UsedEnergy_mWh -ne $null) {
            $energyLine = ", used {0:F2}mWh, {1}" -f $UsedEnergy_mWh, $mWh_per_build
        }

        $overallLine = "$currentTimestamp, run $runCountStr, $CurrentEnergyMode, battery $($batt.BatteryPercent)%, total build $totalBuildStr, clean $totalCleanStr, sleep $totalSleepStr$energyLine"
        Write-And-LogSummary $overallLine

        $secondLine = "   Sleeping ${PauseBetweenRuns}s between builds. $lastBuildLine"
        Write-And-LogSummary $secondLine

        # If on AC, we won't show battery-drop-based stats.
        $showBatteryDropStats = (-not $batt.OnAC) -and ($batteryDropFloat -gt 0)

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
            $timeFor70Drop = "N/A"
            if ($showBatteryDropStats) {
                $buildsPerDropVal = $m.BuildCount / $batteryDropFloat
                $buildsPerDrop = ("{0:F2}" -f $buildsPerDropVal)

                $buildTimePerDropVal = $m.TotalBuildTime.TotalSeconds / $batteryDropFloat
                $buildTimePerDrop = ("{0:F0}s" -f $buildTimePerDropVal)

                $wallPerDropVal = $m.TotalModeTime.TotalSeconds / $batteryDropFloat
                $wallPerDrop = ("{0:F0}s" -f $wallPerDropVal)

                # 70% drop wall estimate
                $wallFor70PctSec = $m.TotalModeTime.TotalSeconds / $batteryDropFloat * 70
                $wallFor70Pct = [TimeSpan]::FromSeconds($wallFor70PctSec)
                $timeFor70Drop = Format-HoursMinutes $wallFor70Pct
            }

            $modeLine = "   On $($m.Mode) for $(Format-ShortTime $m.TotalModeTime), avg build $avgBuild, avg clean $avgClean"
            if ($showBatteryDropStats) {
                $modeLine += ", est. builds per % drop: $buildsPerDrop, build time per %: $buildTimePerDrop, wall per %: $wallPerDrop, 70% drop wall: $timeFor70Drop"
            }
            Write-And-LogSummary $modeLine
        }

        # Write cargo outputs to full log only (already logged summary lines above)
        $fullOutput = "=== Iteration $CompileCount ===`r`n" +
                      "--- Cargo clean output ---`r`n" + ($cleanResult.Output -join "`r`n") + "`r`n" +
                      "--- Cargo build output ---`r`n" + ($buildResult.Output -join "`r`n") + "`r`n"
        Write-FullOnly $fullOutput
    }
} finally {
    # Restore previous system state
    [SleepPreventer.Power]::SetThreadExecutionState($ES_CONTINUOUS) | Out-Null
}

Write-And-LogSummary "Script ended."
