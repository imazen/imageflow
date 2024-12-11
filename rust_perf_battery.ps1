<#
.SYNOPSIS
    Continuously runs `cargo clean` followed by `cargo build` in a loop.
    Logs comprehensive performance and battery statistics per iteration.
    Prevents the system from sleeping or turning off the display during execution,
    and restores the previous system state on script exit.
    Detects large time jumps (sleep/resume) and exits if found.
    Uses high-precision tracking of battery drop per mode.
    Configures Energy Saver thresholds dynamically.
    Reports CPU speed and screen brightness in logs.
    Displays banners on screen brightness or mode changes.
    Reports system uptime and RAM usage at script start.
    Supports configurable sleep intervals and argument passing to `cargo build`.
    Includes comprehensive documentation and comments.
    Keeps API calls and format strings within the summary log.

.DESCRIPTION
    - Runs a self-test before starting. If self-test fails, prints error and exits.
    - On start, sets Energy Saver to activate at 0% battery.
    - Prevents the computer from sleeping or turning off the display by using Win32 API SetThreadExecutionState.
      Uses 64-bit values for ES_CONTINUOUS etc.
    - On start, prints initial line with date/time, battery, screen brightness, CPU speed, system uptime, RAM usage, and memory.
    - For each iteration:
        * Executes cargo clean then cargo build with forwarded arguments.
        * Sleeps a configured interval between runs.
        * Tracks performance stats, including battery usage per mode.
        * Calculates estimated builds per % battery drop, build time per %, wall time per %,
          using a 70% drop wall time estimate per mode.
        * Monitors CPU speed and screen brightness, displaying banners on changes.
        * If iteration takes too long (>3x baseline time), assumes sleep/resume and exits.
    - On command failure, prints the error output and stops.
    - On script end (or if killed), restores Energy Saver threshold to 30% and previous system state.
    - The summary log matches exactly what's printed to terminal line-by-line.
    - The full log includes all summary lines plus cargo outputs.

.NOTES
    Requires PowerShell 5.1+ and `cargo` in PATH.
    Assumes the current directory has a Cargo project (Cargo.toml).

.EXAMPLE
    .\rust_perf_battery.ps1 --sleep 15 --release
#>

param(
    [Parameter(Position=0, Mandatory=$false)]
    [int]$SleepInterval = 10,

    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$CargoArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Configuration
$PauseBetweenRuns = $SleepInterval # seconds to sleep between each iteration
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
            LastScreenBrightness = $null
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

function Get-ScreenBrightness {
    try {
        $brightnessObj = Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightness -ErrorAction Stop
        if ($brightnessObj -and $brightnessObj.CurrentBrightness) {
            return "$($brightnessObj.CurrentBrightness)%"
        } else {
            return "N/A"
        }
    } catch {
        return "N/A"
    }
}

function Get-CPUSpeed {
    try {
        $cpu = Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop
        if ($cpu) {
            $currentClock = $cpu.CurrentClockSpeed
            $maxClock = $cpu.MaxClockSpeed
            if ($maxClock -gt 0) {
                $cpuPercent = [math]::Round(($currentClock / $maxClock) * 100, 0)
                return "$cpuPercent%"
            } else {
                return "N/A"
            }
        } else {
            return "N/A"
        }
    } catch {
        return "N/A"
    }
}

function Get-SystemUptime {
    try {
        $uptime = (Get-CimInstance -ClassName Win32_OperatingSystem).LastBootUpTime
        $uptimeSpan = (Get-Date) - $uptime
        return $uptimeSpan.ToString("dd\.hh\:mm\:ss")
    } catch {
        return "N/A"
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
        Test-Process-Functions
        
    } catch {
        Write-Host "Self-test failed: $($_.Exception.Message)"
        exit 1
    }
}

function Test-Process-Functions {
    $s = Get-ProcessStats
    $t = Get-Date
    $diff = Get-ProcessDiff $s $t
    $table = Format-ProcessDiffTable $diff
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
    $output = & cargo build @CargoArgs 2>&1
    $exit = $LASTEXITCODE
    $elapsed = (Get-Date) - $start
    return [PSCustomObject]@{
        Output = $output
        Success = ($exit -eq 0)
        Elapsed = $elapsed
    }
}

# Logging Functions
function Write-And-LogSummary {
    param([string]$Line)
    Write-Host $Line
    Add-Content $SummaryLogPath $Line
    Add-Content $FullLogPath $Line
}

function Write-FullOnly {
    param([string]$FullOutput)
    Add-Content $FullLogPath $FullOutput
}

# Function to get current energy mode
function Get-CurrentEnergyMode {
    $battery = Get-BatteryStatus
    $schemeName = Get-PowerSchemeName
    $powerSource = if ($battery.OnAC) { "ac" } else { "battery" }
    $modeName = ($schemeName -replace '\s+', '-').ToLower()

    return "$powerSource-$modeName"
}


function Get-RAMUsage {
    try {
        $os = Get-CimInstance Win32_OperatingSystem
        if ($os.TotalVisibleMemorySize -gt 0) {
            return [math]::Round((($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / $os.TotalVisibleMemorySize) * 100, 0)
        }
        return "N/A"
    } catch {
        return "N/A"
    }
}

function Get-ProcessStats {
    param([int]$MinProcessCPUSeconds = 10)
    Get-Process | Where-Object {
        $_.CPU -gt $MinProcessCPUSeconds
    } | ForEach-Object {
        @{
            Name = $_.ProcessName
            CPU = $_.CPU
            WorkingSet = $_.WorkingSet64
            Threads = $_.Threads.Count
            Handles = $_.HandleCount
            ID = if ($_.Id) { $_.Id } else { "unknown" }
        }
    }
}

function Get-ProcessDiff {
    param([Array]$previous, [DateTime]$previousTaken)
    
    $current = Get-ProcessStats
    $currentTaken = (Get-Date)
    $diff = @()
    $totalCpuDelta = 0
    
    foreach ($proc in $current) {
        $prevProc = $previous | Where-Object { $_.ID -eq $proc.ID }
        $cpuDelta = if ($prevProc) { $proc.CPU - $prevProc.CPU } else { $proc.CPU }

        $totalCpuDelta += $cpuDelta
        if ($cpuDelta -gt 0) {
            $diff += @{
                Name = $proc.Name
                CPUDelta = $cpuDelta
                CurrentCPU = $proc.CPU
                WorkingSet = $proc.WorkingSet
                ID = $proc.ID
            }
        }
    }

    $percentCpuDelta = $totalCpuDelta / ($currentTaken - $previousTaken).TotalSeconds
    
    return @{
        CurrentProcesses = $current
        CurrentTaken = $currentTaken
        PreviousProcesses = $previous
        PreviousTaken = $previousTaken
        TotalCpuDelta = $totalCpuDelta
        PercentCpuDelta = $percentCpuDelta
        ElapsedTime = ($currentTaken - $previousTaken)
        Changes = ($diff | Sort-Object -Property CPUDelta -Descending)
    }
}

function Format-ProcessDiffTable {
    param([PSCustomObject]$diff)
    
    if ($diff.TotalCpuDelta -eq 0) {
        return "No change to process cpu usage detected"
    }
    # if percent cpu delta is less than 1%, then don't show it, unless ElapsedTime is under 10 minns
    if ($diff.PercentCpuDelta -lt 1 -and $diff.ElapsedTime.TotalMinutes -gt 10) {
        return "Less than 1% overall CPU usage detected in long-running processes, skipping CPU table display"
    }

    $totalCpuDelta = "{0:N1}s" -f $diff.TotalCpuDelta
    $percentCpuDelta = "{0:N1}%" -f $diff.PercentCpuDelta
    $timeDelta = Format-ShortTime $diff.ElapsedTime
    
    $text = "CPU use $totalCpuDelta ($percentCpuDelta) over $timeDelta (among long-running processes), top 5:`r`n"
    $text += $diff.Changes | 
        Select-Object -First 5 | 
        ForEach-Object {
            [PSCustomObject]@{
                Name = $_.Name
                CPUDelta = "{0:N1}s" -f $_.CPUDelta
                Pct = "{0:N1}%" -f ($_.CPUDelta / $diff.ElapsedTime.TotalSeconds)
                RAMMb = "{0:N0}" -f ($_.WorkingSet / 1MB)
                PID = $_.ID
            }
        } | 
        Format-Table -Property @(
            @{Label="NAME"; Expression={$_.Name}; Align="Left"},
            @{Label="+CPU TIME"; Expression={$_.CPUDelta}; Align="Right"},
            @{Label="% CPU"; Expression={$_.Pct}; Align="Right"},
            @{Label="RAM MB"; Expression={$_.RAMMb}; Align="Right"},
            @{Label="PID"; Expression={$_.PID}; Align="Right"}
        ) -AutoSize | 
        Out-String

    $text += "`r`n"
    ## Now, a single liner with top RAM users
    $text += "Top RAM users: " + ($diff.CurrentProcesses | Sort-Object -Property WorkingSet -Descending | Select-Object -First 5 | ForEach-Object { "$($_.Name) ($($_.WorkingSet / 1MB) MB)" } | Join-String -Separator ", ")
    return $text
}

function Get-TopRAMUsersString {
    return "Top RAM users: " + (Get-Process | Sort-Object -Property WorkingSet -Descending | Select-Object -First 5 | ForEach-Object { "$($_.Name) ($($_.WorkingSet / 1MB) MB)" } | Join-String -Separator ", ")
}

function Get-SystemStatusString {
    $battery = Get-BatteryStatus
    return "battery $($battery.BatteryPercent)%, screen $(Get-ScreenBrightness), cpu speed $(Get-CPUSpeed), ram $(Get-RAMUsage)%"
}


# Prevent sleep/display off
try {
    Add-Type -Namespace SleepPreventer -Name Power -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("kernel32.dll", CharSet=System.Runtime.InteropServices.CharSet.Auto,SetLastError=true)]
public static extern System.UInt32 SetThreadExecutionState(System.UInt64 esFlags);
"@
} catch {
    Write-Host "Failed to add SleepPreventer Power type."
    exit 1
}

[uint64]$ES_CONTINUOUS = 2147483648
[uint64]$ES_SYSTEM_REQUIRED = 1
[uint64]$ES_DISPLAY_REQUIRED = 2

# Set Energy Saver threshold to 0% at start
powercfg /setdcvalueindex SCHEME_CURRENT SUB_ENERGYSAVER ESBATTTHRESHOLD 0
powercfg /setactive SCHEME_CURRENT
Write-Host "Setting Energy Saver to activate at 0% battery."

# Register cleanup to restore Energy Saver threshold
$cleanup = {
    try {
        powercfg /setdcvalueindex SCHEME_CURRENT SUB_ENERGYSAVER ESBATTTHRESHOLD 30
        powercfg /setactive SCHEME_CURRENT
        [SleepPreventer.Power]::SetThreadExecutionState($ES_CONTINUOUS) | Out-Null
        Write-And-LogSummary "Setting Energy Saver to activate at 30% battery."
    } catch {
        Write-Host "Failed to restore Energy Saver settings: $_"
    }
}

# Suppress the output of Register-EngineEvent by piping to Out-Null
Register-EngineEvent PowerShell.Exiting -Action $cleanup | Out-Null

# Prevent system sleep
[SleepPreventer.Power]::SetThreadExecutionState($ES_CONTINUOUS -bor $ES_SYSTEM_REQUIRED -bor $ES_DISPLAY_REQUIRED) | Out-Null

Self-Test
$ModeStats.Clear()

# Initial System Metrics
$batt = Get-BatteryStatus
$initBattery = $batt
$InitialBatteryPercent = $initBattery.BatteryPercent

$initialLine = "$(Get-Date -Format $displayTimeFormat) $(Get-CurrentEnergyMode), $(Get-SystemStatusString), sys uptime $(Get-SystemUptime)"
Write-And-LogSummary $initialLine
Write-And-LogSummary "Warning: 'balanced' may not be accurate, as powercfg /getactivescheme is a lying liar. Verify your power settings manually."
Write-And-LogSummary (Get-TopRAMUsersString)

$logLine = "Logs: $(Split-Path $FullLogPath -Leaf), $(Split-Path $SummaryLogPath -Leaf)"
Write-And-LogSummary $logLine

# Add these near the top with other global variables
$LastProcessCheck = [DateTime]::MinValue
$BaselineProcessStats = Get-ProcessStats
$BaselineProcessCheck = Get-Date
$ProcessCheckInterval = [TimeSpan]::FromMinutes(5)



try {
    while (-not $StopLoop) {
        $iterationStart = Get-Date
        $CurrentEnergyMode = Get-CurrentEnergyMode
        $currentScreenBrightness = Get-ScreenBrightness
        

        # Check for mode or screen brightness changes
        if ($ModeStats.ContainsKey($CurrentEnergyMode)) {
            $lastBrightness = $ModeStats[$CurrentEnergyMode].LastScreenBrightness
            if ($lastBrightness -ne $currentScreenBrightness) {
                Write-And-LogSummary "=== Warning: Screen brightness changed from $lastBrightness to $currentScreenBrightness ==="
                $ModeStats[$CurrentEnergyMode].LastScreenBrightness = $currentScreenBrightness
            }
            
            # Add mode change warning
            if ($PreviousMode -and $PreviousMode -ne $CurrentEnergyMode) {
                Write-And-LogSummary "=== Warning: Power mode changed from $PreviousMode to $CurrentEnergyMode ==="
            }
        }
        $PreviousMode = $CurrentEnergyMode

        Ensure-ModeStats($CurrentEnergyMode)
        $ModeStats[$CurrentEnergyMode].LastScreenBrightness = $currentScreenBrightness

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
        $otherTimeStr = Format-ShortTime $otherTime

        $totalBuildStr = Format-ShortTime $TotalBuildTime
        $totalCleanStr = Format-ShortTime $TotalCleanTime
        $totalSleepStr = Format-ShortTime $TotalSleepTime

        $batteryDrop = $InitialBatteryPercent - $batt.BatteryPercent
        $batteryDropFloat = [double]$batteryDrop
        $currentTimestamp = (Get-Date -Format $displayTimeFormat)
        $runCountStr = $CompileCount

        
        $overallLine = "$currentTimestamp, run $runCountStr, $CurrentEnergyMode, $(Get-SystemStatusString), total build $totalBuildStr, clean $totalCleanStr, sleep $totalSleepStr, other $otherTimeStr"
        Write-And-LogSummary $overallLine

        $secondLine = "   Sleeping ${PauseBetweenRuns}s between builds. $lastBuildLine"
        Write-And-LogSummary $secondLine

        # Show battery drop stats only if on battery
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
            if ($showBatteryDropStats -and $batteryDropFloat -ne 0) {
                $buildsPerDropVal = $m.BuildCount / $batteryDropFloat
                $buildsPerDrop = ("{0:F2}" -f $buildsPerDropVal)

                $buildTimePerDropVal = $m.TotalBuildTime.TotalSeconds / $batteryDropFloat
                $buildTimePerDrop = ("{0:F0}s" -f $buildTimePerDropVal)

                $wallPerDropVal = $m.TotalModeTime.TotalSeconds / $batteryDropFloat
                $wallPerDrop = ("{0:F0}s" -f $wallPerDropVal)

                # 70% drop wall estimate
                $wallFor70PctSec = ($m.TotalModeTime.TotalSeconds / $batteryDropFloat) * 70
                $wallFor70Pct = [TimeSpan]::FromSeconds($wallFor70PctSec)
                $timeFor70Drop = Format-HoursMinutes $wallFor70Pct
            }

            $modeLine = "   On $($m.Mode) for $(Format-ShortTime $m.TotalModeTime), avg build $avgBuild, avg clean $avgClean"
            if ($showBatteryDropStats -and $batteryDropFloat -ne 0) {
                $modeLine += ", est. builds per % drop: $buildsPerDrop, build time per %: $buildTimePerDrop, wall per %: $wallPerDrop, est. $timeFor70Drop to drop 70% based on $batteryDropFloat% over $(Format-ShortTime $m.TotalModeTime)"
            }
            Write-And-LogSummary $modeLine
        }

        # Write cargo outputs to full log only
        $fullOutput = "=== Iteration $CompileCount ===`r`n" +
                      "--- Cargo clean output ---`r`n" + ($cleanResult.Output -join "`r`n") + "`r`n" +
                      "--- Cargo build output ---`r`n" + ($buildResult.Output -join "`r`n") + "`r`n"
        Write-FullOnly $fullOutput

        if (((Get-Date) - $LastProcessCheck) -gt $ProcessCheckInterval -or $CompileCount -eq 1) {
            $diff = Get-ProcessDiff $BaselineProcessStats $BaselineProcessCheck
            $table = Format-ProcessDiffTable $diff
            Write-And-LogSummary $table
            $LastProcessCheck = Get-Date
        }
    }
} finally {
    # Cleanup actions
    $cleanup.Invoke()
    Write-And-LogSummary "Script ended."
}
