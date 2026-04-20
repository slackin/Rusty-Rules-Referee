#!/usr/bin/env pwsh
# =============================================================================
# push-update.ps1 — Push a new R3 build to the update server (r3.pugbot.net)
#
# This script:
#   1. Takes a compiled binary (or builds one)
#   2. Extracts the build hash from the binary
#   3. Computes SHA-256 of the binary
#   4. Generates latest.json manifest
#   5. Uploads both to r3.pugbot.net via scp
#
# Usage:
#   $env:DEPLOY_PASS = "yourpass"; .\push-update.ps1
#   $env:DEPLOY_PASS = "yourpass"; .\push-update.ps1 -Binary path\to\binary
#   $env:DEPLOY_PASS = "yourpass"; .\push-update.ps1 -Build
#
# Requires: scp (OpenSSH), sshpass (WSL) or ssh key auth
# =============================================================================

param(
    [string]$Binary = "",
    [string]$Platform = "linux-x86_64",
    [switch]$Build,
    [switch]$Help
)

$ErrorActionPreference = 'Stop'

# ---- Config ----
$RemoteUser = "root"
$RemoteHost = "10.10.0.4"
$RemoteBase = "/home/bcmx/domains/r3.pugbot.net/public_html/api/updates"
$ScriptDir = $PSScriptRoot

# Release channels: production, beta, alpha, dev.
# This script ALWAYS publishes to the dev channel. Use promote.sh on the
# update server to move builds between channels (dev -> alpha -> beta -> production).
$Channel = "dev"
$RemoteDir = "${RemoteBase}/${Channel}"

if ($Help) {
    Write-Host "Usage: `$env:DEPLOY_PASS = 'pass'; .\push-update.ps1 [-Binary path] [-Platform name] [-Build]"
    Write-Host ""
    Write-Host "  -Binary PATH     Path to compiled binary (default: target/release/rusty-rules-referee)"
    Write-Host "  -Platform NAME   Platform identifier (default: linux-x86_64)"
    Write-Host "  -Build           Build the binary first (runs build.ps1)"
    exit 0
}

if (-not $env:DEPLOY_PASS) {
    Write-Host "  ERROR: DEPLOY_PASS environment variable is not set." -ForegroundColor Red
    Write-Host "  Usage: `$env:DEPLOY_PASS = 'yourpass'; .\push-update.ps1"
    exit 1
}

# ---- Build if requested ----
if ($Build) {
    Write-Host ""
    Write-Host "=== Building R3 ===" -ForegroundColor Cyan
    & "$ScriptDir\build.ps1"
    Write-Host ""
}

# ---- Resolve binary path ----
if (-not $Binary) {
    $Binary = Join-Path $ScriptDir "target/release/rusty-rules-referee"
}

if (-not (Test-Path $Binary)) {
    Write-Host "  ERROR: Binary not found: $Binary" -ForegroundColor Red
    Write-Host "  Build first with: .\build.ps1"
    exit 1
}

Write-Host ""
Write-Host "=== Push R3 Update ===" -ForegroundColor Cyan
Write-Host ""

# ---- Extract build hash from binary ----
Write-Host "  Extracting build hash..." -ForegroundColor Green
$BuildHash = & $Binary --build-hash 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "  ERROR: Failed to extract build hash." -ForegroundColor Red
    exit 1
}
$BuildHash = $BuildHash.Trim()
Write-Host "  Build hash: $BuildHash" -ForegroundColor Green

# Parse version and git commit
$Parts = $BuildHash -split '-'
$Version = $Parts[0]
$GitCommit = $Parts[1]
Write-Host "  Version: $Version, Git: $GitCommit" -ForegroundColor Green

# ---- Compute SHA-256 ----
Write-Host "  Computing SHA-256..." -ForegroundColor Green
$Sha256 = (Get-FileHash -Path $Binary -Algorithm SHA256).Hash.ToLower()
Write-Host "  SHA-256: $Sha256" -ForegroundColor Green

# ---- Get file size ----
$FileSize = (Get-Item $Binary).Length
$FileSizeMB = [math]::Round($FileSize / 1MB, 2)
Write-Host "  Size: $FileSize bytes (${FileSizeMB}MB)" -ForegroundColor Green

# ---- Generate latest.json ----
$BinaryFilename = "r3-${Platform}"
$DownloadUrl = "https://r3.pugbot.net/api/updates/${Channel}/binaries/${BinaryFilename}"
$ReleasedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$Manifest = [ordered]@{
    channel    = $Channel
    version    = $Version
    build_hash = $BuildHash
    git_commit = $GitCommit
    released_at = $ReleasedAt
    platforms  = @{
        $Platform = @{
            url    = $DownloadUrl
            sha256 = $Sha256
            size   = $FileSize
        }
    }
} | ConvertTo-Json -Depth 4

Write-Host ""
Write-Host "  Manifest:" -ForegroundColor Green
$Manifest | ForEach-Object { Write-Host "    $_" }
Write-Host ""

# ---- Upload to server ----
# Write manifest to temp file
$TempManifest = [System.IO.Path]::GetTempFileName()
$Manifest | Set-Content -Path $TempManifest -Encoding UTF8 -NoNewline

try {
    Write-Host "  Creating remote directories (channel: ${Channel})..." -ForegroundColor Green
    ssh -o StrictHostKeyChecking=no "${RemoteUser}@${RemoteHost}" "mkdir -p ${RemoteDir}/binaries"

    Write-Host "  Uploading binary (${BinaryFilename})..." -ForegroundColor Green
    scp -o StrictHostKeyChecking=no $Binary "${RemoteUser}@${RemoteHost}:${RemoteDir}/binaries/${BinaryFilename}"

    Write-Host "  Uploading latest.json..." -ForegroundColor Green
    scp -o StrictHostKeyChecking=no $TempManifest "${RemoteUser}@${RemoteHost}:${RemoteDir}/latest.json"

    Write-Host "  Setting permissions..." -ForegroundColor Green
    ssh -o StrictHostKeyChecking=no "${RemoteUser}@${RemoteHost}" "chmod 644 ${RemoteDir}/latest.json ${RemoteDir}/binaries/${BinaryFilename}"
}
finally {
    Remove-Item $TempManifest -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "  Update pushed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "  Channel:  $Channel"
Write-Host "  Build:    $BuildHash"
Write-Host "  Manifest: https://r3.pugbot.net/api/updates/${Channel}/latest.json"
Write-Host "  Binary:   $DownloadUrl"
Write-Host ""
Write-Host "  Bots on the '$Channel' channel with [update] enabled = true will pick this up automatically."
Write-Host "  Use promote.sh on the update server to move this build to alpha/beta/production."
Write-Host ""
