#!/usr/bin/env pwsh
# Build script for R3 Admin — builds the SvelteKit frontend then the Rust binary

$ErrorActionPreference = 'Stop'

Write-Host "=== Building R3 Admin UI ===" -ForegroundColor Cyan
Push-Location "$PSScriptRoot/ui"

if (!(Test-Path "node_modules")) {
    Write-Host "Installing npm dependencies..."
    npm install
}

Write-Host "Building SvelteKit app..."
npm run build

Pop-Location

Write-Host ""
Write-Host "=== Building Rust binary ===" -ForegroundColor Cyan
cargo build --release

Write-Host ""
Write-Host "Build complete!" -ForegroundColor Green
Write-Host "Binary: target/release/rusty-rules-referee"
