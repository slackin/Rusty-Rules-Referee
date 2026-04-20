#!/usr/bin/env pwsh
# =============================================================================
# deploy.ps1 — Deploy R3 from Windows (PowerShell)
#
# This script:
#   1. Checks for uncommitted changes and auto-commits
#   2. Pushes to GitHub
#   3. SSHs to the build server (r3.pugbot.net) and runs deploy-remote.sh
#   4. The build server pulls, builds, and publishes the update
#   5. Game servers auto-update from the published manifest
#
# Usage:
#   .\deploy.ps1                       # auto-commit with default message
#   .\deploy.ps1 "fix censor bug"      # auto-commit with custom message
#   .\deploy.ps1 -NoCommit             # skip commit (must be pushed already)
#
# Prerequisites:
#   - SSH key auth to root@10.10.0.4 (r3.pugbot.net)
#   - Build server set up via setup-build-server.sh
# =============================================================================

param(
    [Parameter(Position = 0)]
    [string]$Message = "",
    [switch]$NoCommit,
    [switch]$Help
)

$ErrorActionPreference = 'Stop'

# ---- Config ----
$BuildServer = "root@10.10.0.4"
$BuildDir = "/opt/r3-build"
$RequiredBranch = "main"

if ($Help) {
    Write-Host 'Usage: .\deploy.ps1 ["commit message"] [-NoCommit]'
    Write-Host '  "message"     Custom commit message (default: "deploy: vX.Y.Z")'
    Write-Host '  -NoCommit     Skip auto-commit, just push and build'
    exit 0
}

function Write-Step($text)  { Write-Host "`n=== $text ===" -ForegroundColor Cyan }
function Write-Ok($text)    { Write-Host "  OK  $text" -ForegroundColor Green }
function Write-Warn($text)  { Write-Host "  !!  $text" -ForegroundColor Yellow }

function Stop-Deploy($text) {
    Write-Host "  FAIL  $text" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "  ======================================" -ForegroundColor Cyan
Write-Host "         R3 Deploy Pipeline             " -ForegroundColor Cyan
Write-Host "  ======================================" -ForegroundColor Cyan
Write-Host ""

# ---- Pre-flight: Verify git repo ----
if (-not (Test-Path ".git")) {
    Stop-Deploy "Not in a git repository. Run from the R3 project root."
}

# ---- Pre-flight: Check branch ----
Write-Step "Pre-flight checks"
$Branch = git rev-parse --abbrev-ref HEAD
if ($Branch -ne $RequiredBranch) {
    Write-Warn "On branch '$Branch' (expected '$RequiredBranch')"
    $reply = Read-Host "  Deploy from '$Branch' anyway? [y/N]"
    if ($reply -notin @('y', 'Y')) {
        Stop-Deploy "Aborted. Switch to $RequiredBranch first."
    }
}
Write-Ok "Branch: $Branch"

# ---- Pre-flight: Check SSH connectivity ----
$sshTest = ssh -o ConnectTimeout=5 -o BatchMode=yes $BuildServer "echo ok" 2>&1
if ($LASTEXITCODE -ne 0) {
    Stop-Deploy "Cannot SSH to $BuildServer — set up SSH key auth first (see setup-build-server.sh)"
}
Write-Ok "SSH to ${BuildServer}: connected"

# ---- Step: Auto-commit ----
Write-Step "Git commit & push"

if (-not $NoCommit) {
    $status = git status --porcelain
    if ($status) {
        # Get version from Cargo.toml
        $versionLine = Select-String -Path "Cargo.toml" -Pattern '^version' | Select-Object -First 1
        $Version = ($versionLine -replace '.*"(.*)".*', '$1').ToString().Trim()
        # Clean up the version string from the match
        if ($versionLine.Line -match '"([^"]+)"') {
            $Version = $Matches[1]
        }

        if ($Message) {
            $commitMsg = "deploy: v${Version} — ${Message}"
        } else {
            $commitMsg = "deploy: v${Version}"
        }

        git add -A
        git commit -m $commitMsg
        if ($LASTEXITCODE -ne 0) { Stop-Deploy "git commit failed" }
        Write-Ok "Committed: $commitMsg"
    } else {
        Write-Ok "Working tree clean, nothing to commit"
    }
} else {
    Write-Ok "Skipping commit (-NoCommit)"
}

# ---- Step: Push to GitHub ----
git push origin $Branch
if ($LASTEXITCODE -ne 0) { Stop-Deploy "git push failed — resolve conflicts first" }
$Commit = git rev-parse --short=8 HEAD
Write-Ok "Pushed $Commit to origin/$Branch"

# ---- Step: Build & publish on update server ----
Write-Step "Building on r3.pugbot.net"
Write-Host ""

# Source cargo env on the remote in case it's not in the default login shell PATH
ssh -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 -o ServerAliveCountMax=60 $BuildServer "bash ${BuildDir}/deploy-remote.sh"
if ($LASTEXITCODE -ne 0) {
    Stop-Deploy "Remote build failed — SSH to $BuildServer and check ${BuildDir}/deploy-remote.sh"
}

# ---- Done ----
Write-Host ""
Write-Host "  Deploy complete! Game servers will auto-update." -ForegroundColor Green
Write-Host ""
