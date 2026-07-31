#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Require-Command {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [string] $InstallHint = ""
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        Write-Error "Required command not found in PATH: $Name"
        if ($InstallHint) {
            Write-Host "Install hint: $InstallHint"
        }
        exit 1
    }
}

$scriptDir = Split-Path -Parent $PSCommandPath
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
Set-Location $repoRoot

Require-Command -Name "cargo"
Require-Command -Name "wasm-bindgen" -InstallHint "cargo install wasm-bindgen-cli"

Write-Host "Building pocket-tts for wasm32-unknown-unknown (release)..."
& cargo build -p pocket-tts --release --target wasm32-unknown-unknown --features wasm

$targetBase = if ($env:CARGO_TARGET_DIR -and $env:CARGO_TARGET_DIR.Trim().Length -gt 0) {
    $env:CARGO_TARGET_DIR
} else {
    "target"
}

$candidates = @(
    (Join-Path $targetBase "wasm32-unknown-unknown\release\pocket_tts.wasm"),
    (Join-Path "target" "wasm32-unknown-unknown\release\pocket_tts.wasm"),
    "D:\RustBuilds\wasm32-unknown-unknown\release\pocket_tts.wasm"
) | Select-Object -Unique

$wasmPath = $null
foreach ($candidate in $candidates) {
    if (Test-Path -LiteralPath $candidate) {
        $wasmPath = (Resolve-Path -LiteralPath $candidate).Path
        break
    }
}

if (-not $wasmPath) {
    Write-Error "Could not find pocket_tts.wasm after build."
    Write-Host "Checked paths:"
    foreach ($candidate in $candidates) {
        Write-Host "  - $candidate"
    }
    exit 1
}

$outDir = Join-Path $repoRoot "crates\pocket-tts\pkg"
New-Item -ItemType Directory -Path $outDir -Force | Out-Null

Write-Host "Running wasm-bindgen..."
& wasm-bindgen --target web --out-dir $outDir $wasmPath

$bgWasm = Join-Path $outDir "pocket_tts_bg.wasm"
if (Get-Command "wasm-opt" -ErrorAction SilentlyContinue) {
    Write-Host "Running wasm-opt -O3..."
    & wasm-opt -O3 --enable-mutable-globals -o $bgWasm $bgWasm
} else {
    Write-Warning "wasm-opt not found. Skipping optimization step."
}

Write-Host ""
Write-Host "WASM build complete."
Write-Host "Artifacts:"
Write-Host "  - crates/pocket-tts/pkg/pocket_tts.js"
Write-Host "  - crates/pocket-tts/pkg/pocket_tts_bg.wasm"
