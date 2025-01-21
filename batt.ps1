# PowerShell Script to Assess Laptop Battery Efficiency

# Function to Get Battery Information
function Get-BatteryInfo {
    $battery = Get-CimInstance -ClassName Win32_Battery
    if ($battery) {
        $batteryStatus = switch ($battery.BatteryStatus) {
            1 { "Discharging" }
            2 { "Charging" }
            3 { "Fully Charged" }
            default { "Unknown" }
        }
        return @{
            "Charge Percentage"      = "$($battery.EstimatedChargeRemaining)%"
            "Charging Status"        = $batteryStatus
            "Design Capacity (mWh)"  = $battery.DesignCapacity
            "Full Charge Capacity (mWh)" = $battery.FullChargeCapacity
            "Battery Health (%)"     = "{0:N2}" -f (($battery.FullChargeCapacity / $battery.DesignCapacity) * 100)
        }
    } else {
        return @{
            "Battery Info" = "No battery detected."
        }
    }
}

# Function to Get Screen Brightness
function Get-ScreenBrightness {
    $brightness = (Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightness).CurrentBrightness
    return "$brightness%"
}

# Function to Get CPU Information
function Get-CPUInfo {
    $cpu = Get-CimInstance -ClassName Win32_Processor
    $currentClock = $cpu.CurrentClockSpeed
    $maxClock = $cpu.MaxClockSpeed
    $throttling = if ($currentClock -lt ($maxClock * 0.95)) { "Yes" } else { "No" }

    return @{
        "Current CPU Frequency (MHz)" = "$currentClock MHz"
        "Max CPU Frequency (MHz)"     = "$maxClock MHz"
        "CPU Throttling Active"      = $throttling
    }
}

# Function to Get Power Plan
function Get-PowerPlan {
    $powerPlan = powercfg /getactivescheme
    if ($powerPlan) {
        return ($powerPlan -split ': ')[1]
    } else {
        return "Unknown"
    }
}

# Function to Get System Uptime
function Get-SystemUptime {
    $uptime = (Get-CimInstance -ClassName Win32_OperatingSystem).LastBootUpTime
    $uptimeSpan = (Get-Date) - $uptime
    return $uptimeSpan.ToString("dd\.hh\:mm\:ss")
}

# Function to Get Top Power-Consuming Processes
function Get-TopProcesses {
    # Requires PowerShell 5.1 or later
    $processes = Get-Process | Sort-Object -Property @{Expression={$_.PM + $_.WS}} -Descending | Select-Object -First 5 | 
        Select-Object Name, @{Name="Memory (MB)";Expression={[math]::round($_.PM / 1MB,2)}}, @{Name="Handles";Expression={$_.Handles}}
    return $processes
}

# Collect All Information
$batteryInfo = Get-BatteryInfo
$brightness = Get-ScreenBrightness
$cpuInfo = Get-CPUInfo
$powerPlan = Get-PowerPlan
$uptime = Get-SystemUptime
$topProcesses = Get-TopProcesses

# Display Summary
Write-Output "===== Laptop Battery Efficiency Report =====`n"

Write-Output ">> Battery Information:"
$batteryInfo.GetEnumerator() | ForEach-Object { Write-Output "   $_.Key : $_.Value" }
Write-Output ""

Write-Output ">> Screen Brightness: $brightness`n"

Write-Output ">> CPU Information:"
$cpuInfo.GetEnumerator() | ForEach-Object { Write-Output "   $_.Key : $_.Value" }
Write-Output ""

Write-Output ">> Power Plan: $powerPlan`n"

Write-Output ">> System Uptime: $uptime`n"

Write-Output ">> Top 5 Power-Consuming Processes:"
$topProcesses | Format-Table -AutoSize

Write-Output "`n============================================="
