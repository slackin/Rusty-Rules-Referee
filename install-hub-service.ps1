<#
.SYNOPSIS
    Installs the Rusty Rules Referee (R3) Windows hub as a durable, self-restarting
    Scheduled Task. The task survives reboots and automatically restarts the hub if
    it crashes.

.DESCRIPTION
    A real Windows Service requires a service-aware binary (the R3 hub is a plain
    console app) or a wrapper like NSSM. The most reliable *native* way to keep a
    plain executable running 24/7 - across reboots and crashes - is a Scheduled Task
    configured with an "At Startup" trigger and restart-on-failure settings.

    By default the task runs as the SYSTEM account so it starts at boot without any
    user logged on. Pass -RunAsUser to instead run under the current interactive
    user (you will be prompted for a password so the task can run while logged off).

    MUST be run from an ELEVATED PowerShell (Run as Administrator).

.PARAMETER HubDir
    Directory containing rusty-rules-referee.exe and r3.toml. Default: C:\r3-hub-local

.PARAMETER TaskName
    Name of the scheduled task. Default: R3Hub

.PARAMETER RunAsUser
    Run the task as the current interactive user (prompts for password) instead of
    the SYSTEM account. Use this if the hub/game servers need access to your user
    profile or a desktop session.

.PARAMETER Uninstall
    Remove the scheduled task instead of creating it.

.EXAMPLE
    # Right-click PowerShell -> Run as Administrator, then:
    .\install-hub-service.ps1

.EXAMPLE
    .\install-hub-service.ps1 -RunAsUser

.EXAMPLE
    .\install-hub-service.ps1 -Uninstall
#>
[CmdletBinding()]
param(
    [string]$HubDir   = 'C:\r3-hub-local',
    [string]$TaskName = 'R3Hub',
    [switch]$RunAsUser,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

function Assert-Admin {
    $isAdmin = ([Security.Principal.WindowsPrincipal] `
        [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) {
        Write-Error "This script must be run from an ELEVATED PowerShell (Run as Administrator)."
        exit 1
    }
}

Assert-Admin

# ---- Uninstall path -------------------------------------------------------
if ($Uninstall) {
    if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
        Stop-ScheduledTask  -TaskName $TaskName -ErrorAction SilentlyContinue
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        Write-Host "Removed scheduled task '$TaskName'." -ForegroundColor Green
    } else {
        Write-Host "Scheduled task '$TaskName' not found - nothing to remove." -ForegroundColor Yellow
    }
    # Stop any running hub/client processes started by the task.
    Get-Process -Name 'rusty-rules-referee' -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    exit 0
}

# ---- Validate hub install -------------------------------------------------
$exe = Join-Path $HubDir 'rusty-rules-referee.exe'
$cfg = Join-Path $HubDir 'r3.toml'
if (-not (Test-Path $exe)) { Write-Error "Hub binary not found: $exe"; exit 1 }
if (-not (Test-Path $cfg)) { Write-Error "Hub config not found: $cfg"; exit 1 }

# ---- A small launcher wrapper so stdout/stderr land in a rotating log ------
# The hub writes its primary log itself, but capturing the console stream gives
# us crash/panic output that would otherwise be lost when run headless.
$launcher = Join-Path $HubDir 'run-hub.cmd'
$log = Join-Path $HubDir 'hub-task.log'
$q = [char]34  # double-quote, kept out of string literals to dodge PS parsing
$launcherLines = @(
    '@echo off'
    'cd /d ' + $q + $HubDir + $q
    ':loop'
    'echo [%date% %time%] starting hub ' + '>> ' + $q + $log + $q
    $q + $exe + $q + ' --mode hub r3.toml ' + '>> ' + $q + $log + $q + ' 2' + '>' + '&1'
    'echo [%date% %time%] hub exited, restarting in 5s ' + '>> ' + $q + $log + $q
    'timeout /t 5 /nobreak ' + '>' + ' nul'
    'goto loop'
)
Set-Content -Path $launcher -Value $launcherLines -Encoding ASCII
Write-Host "Wrote launcher: $launcher" -ForegroundColor DarkGray

# ---- Build the scheduled task --------------------------------------------
$action = New-ScheduledTaskAction -Execute 'cmd.exe' `
    -Argument "/c `"$launcher`"" -WorkingDirectory $HubDir

$trigger = New-ScheduledTaskTrigger -AtStartup

# Restart-on-failure + run forever (no execution time limit).
$settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -StartWhenAvailable `
    -RestartCount 999 `
    -RestartInterval (New-TimeSpan -Minutes 1) `
    -ExecutionTimeLimit (New-TimeSpan -Seconds 0)

if ($RunAsUser) {
    $userId = "$env:USERDOMAIN\$env:USERNAME"
    Write-Host "Task will run as user '$userId' (whether logged on or not)." -ForegroundColor Cyan
    $cred = Get-Credential -UserName $userId -Message "Enter password for $userId so the hub can run while logged off"
    $principal = New-ScheduledTaskPrincipal -UserId $userId -LogonType Password -RunLevel Highest
    Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger `
        -Settings $settings -Principal $principal `
        -User $userId -Password $cred.GetNetworkCredential().Password -Force | Out-Null
} else {
    Write-Host "Task will run as the SYSTEM account at startup." -ForegroundColor Cyan
    $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' `
        -LogonType ServiceAccount -RunLevel Highest
    Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger `
        -Settings $settings -Principal $principal -Force | Out-Null
}

Write-Host "Registered scheduled task '$TaskName'." -ForegroundColor Green

# ---- Stop any ad-hoc processes, then start fresh via the task -------------
Get-Process -Name 'rusty-rules-referee' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

Start-ScheduledTask -TaskName $TaskName
Write-Host "Started task. Waiting for the hub to come up..." -ForegroundColor Cyan
Start-Sleep -Seconds 12

$procs = Get-Process -Name 'rusty-rules-referee' -ErrorAction SilentlyContinue
if ($procs) {
    Write-Host "Hub is running:" -ForegroundColor Green
    $procs | Select-Object Id, StartTime | Format-Table | Out-String | Write-Host
    Write-Host "Console log: $HubDir\hub-task.log"
    Write-Host ""
    Write-Host "Done. The hub will now start at boot and auto-restart on crash." -ForegroundColor Green
    Write-Host "To remove:  .\install-hub-service.ps1 -Uninstall"
} else {
    Write-Warning "No hub process detected yet. Check $HubDir\hub-task.log for errors."
}
