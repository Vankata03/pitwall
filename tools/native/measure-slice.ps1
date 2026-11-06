param(
    [Parameter(Mandatory)][string]$Recording,
    [string]$Executable = "$PSScriptRoot/../../target/release/pitwall-native.exe",
    [ValidateRange(5, 120)][int]$Seconds = 30,
    [switch]$KeepDisplayOn,
    [string]$Output = "$env:TEMP/pitwall-slice-measurement.json"
)
$ErrorActionPreference = 'Stop'
$recordingPath = (Resolve-Path -LiteralPath $Recording).Path
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
$runFolder = Join-Path $env:TEMP ("pitwall-slice-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $runFolder | Out-Null
$stdout = Join-Path $runFolder 'stats.txt'
$stderr = Join-Path $runFolder 'stderr.txt'
if ($KeepDisplayOn) {
    Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public static class PitwallMeasurementPower {
    [DllImport("kernel32.dll")]
    public static extern uint SetThreadExecutionState(uint flags);
}
'@
    $previousPowerState = [PitwallMeasurementPower]::SetThreadExecutionState(0x80000003u)
    if ($previousPowerState -eq 0) { throw 'Cannot request an awake display.' }
}
$appProcess = $null
try {
    # The native dashboard is the UI under test, so it must have a visible window.
    $appProcess = Start-Process -FilePath $executablePath -ArgumentList @('--console', '--data-dir', ('"' + $runFolder + '"'), '--replay', ('"' + $recordingPath + '"'), '--from', '120', '--loop', '--stats') -WindowStyle Normal -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    Start-Sleep -Seconds 6
    $appProcess.Refresh()
    if ($appProcess.HasExited) { throw "App exited: $(Get-Content -LiteralPath $stderr -Raw)" }
    $initialCpu = $appProcess.TotalProcessorTime.TotalSeconds
    $initialStatsLines = @(Get-Content -LiteralPath $stdout).Count
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $gpuValues = [Collections.Generic.List[double]]::new()
    $gpuError = $null
    while ($timer.Elapsed.TotalSeconds -lt $Seconds) {
        Start-Sleep -Seconds 1
        $appProcess.Refresh()
        if ($appProcess.HasExited) { throw "App exited: $(Get-Content -LiteralPath $stderr -Raw)" }
        try {
            $counter = Get-Counter -Counter "\GPU Engine(pid_$($appProcess.Id)_*)\Utilization Percentage" -ErrorAction Stop
            $engines = @($counter.CounterSamples | Where-Object { $_.InstanceName -like "pid_$($appProcess.Id)_*" })
            if ($engines.Count -gt 0) {
                $gpuValues.Add(($engines | Measure-Object -Property CookedValue -Sum).Sum)
            } else { $gpuError = 'No GPU engine counters matched the app process.' }
        } catch { $gpuError = $_.Exception.Message }
    }
    $timer.Stop()
    $appProcess.Refresh()
    $cpuOneCore = 100 * ($appProcess.TotalProcessorTime.TotalSeconds - $initialCpu) / $timer.Elapsed.TotalSeconds
    $stats = @(Get-Content -LiteralPath $stdout | Select-Object -Skip $initialStatsLines)
    $renders = @($stats | ForEach-Object { if ($_ -match 'renders/s=(\d+)') { [int]$Matches[1] } })
    $frames = @($stats | ForEach-Object { if ($_ -match 'frames/s=(\d+)') { [int]$Matches[1] } })
    $result = [ordered]@{
        measuredAt = (Get-Date).ToString('o')
        isolatedDataDirectory = $runFolder
        executable = $executablePath
        executableSha256 = (Get-FileHash -LiteralPath $executablePath -Algorithm SHA256).Hash
        recording = [IO.Path]::GetFileName($recordingPath)
        recordingSha256 = (Get-FileHash -LiteralPath $recordingPath -Algorithm SHA256).Hash
        wallSeconds = $timer.Elapsed.TotalSeconds
        logicalProcessors = [Environment]::ProcessorCount
        cpuOneCorePercent = $cpuOneCore
        cpuMachinePercent = $cpuOneCore / [Environment]::ProcessorCount
        workingSetBytes = $appProcess.WorkingSet64
        peakWorkingSetBytes = $appProcess.PeakWorkingSet64
        privateBytes = $appProcess.PrivateMemorySize64
        gpuEngineSumAveragePercent = ($gpuValues | Measure-Object -Average).Average
        gpuEngineSumMaxPercent = ($gpuValues | Measure-Object -Maximum).Maximum
        gpuSamples = $gpuValues.Count
        gpuError = $gpuError
        rendersPerSecond = ($renders | Measure-Object -Average).Average
        rendersPerSecondMin = ($renders | Measure-Object -Minimum).Minimum
        displayFramesPerSecond = ($frames | Measure-Object -Average).Average
        statsFile = $stdout
        stderrFile = $stderr
        keepDisplayOn = [bool]$KeepDisplayOn
        focusAndDisplayPower = 'Focus uncontrolled; inspect render counts before claiming 60 Hz.'
    }
    $result | ConvertTo-Json | Set-Content -LiteralPath $Output
    $result | ConvertTo-Json
} finally {
    if ($null -ne $appProcess -and -not $appProcess.HasExited) { Stop-Process -Id $appProcess.Id }
    if ($KeepDisplayOn) { [PitwallMeasurementPower]::SetThreadExecutionState($previousPowerState) | Out-Null }
}
