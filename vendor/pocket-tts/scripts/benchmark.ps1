# Run full Python vs Rust benchmark suite
# Requirements: hyperfine, cargo, uv

# Check for hyperfine
if (!(Get-Command hyperfine -ErrorAction SilentlyContinue)) {
    Write-Error "hyperfine not found in PATH. Please install it: cargo install hyperfine"
    exit 1
}

$texts = @(
    "Hello world",
    "This is a medium length sentence for benchmarking.",
    "The sun was beginning to set over the horizon, casting a warm golden glow across the quiet valley. A gentle breeze rustled the leaves of the old oak trees, carrying the sweet scent of blooming wildflowers. In the distance, the faint sound of a rushing stream provided a soothing backdrop to the peaceful evening.",
    "Artificial intelligence is rapidly transforming the way we interact with technology and each other. From advanced natural language processing to sophisticated image recognition, these systems are becoming increasingly integrated into our daily lives. As we continue to develop and refine these models, it is crucial to consider the ethical implications and ensure that they are used for the benefit of all humanity. The future of AI holds great promise, but it also requires careful stewardship and a commitment to transparency and accountability."
)

# Setup temporary and final output paths
$tmpDir = ".bench_tmp"
if (!(Test-Path $tmpDir)) { New-Item -ItemType Directory -Path $tmpDir }
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$finalOutput = "benchmark_results_$timestamp.md"

# Ensure release build is up to date
Write-Host "Building Release..."
cargo build --release -p pocket-tts-cli --features mkl

foreach ($i in 0..($texts.Length - 1)) {
    $text = $texts[$i]
    Write-Host "`n=== Benchmark $($i + 1): $($text.Length) chars ===" -ForegroundColor Cyan
    
    # We use uv run with the root venv and point to the local python code
    # Using 'cmd /C' to allow setting environment variables inline on Windows
    $pyCmd = "cmd /C ""set PYTHONPATH=python-reference&& uv run --no-project python -m pocket_tts.main generate --text `"$text`" --output-path bench_py.wav"""
    $rsCmd = ".\target\release\pocket-tts-cli.exe generate --text `"$text`" --output bench_rs.wav"
    
    $exportFile = Join-Path $tmpDir "results_$($i + 1).md"
    
    hyperfine --warmup 1 --runs 3 `
        --command-name "Python (local)" "$pyCmd" `
        --command-name "Rust (release)" "$rsCmd" `
        --export-markdown "$exportFile"
}

Write-Host "`nConcatenating results into $finalOutput..." -ForegroundColor Green
" # Benchmark Results - $timestamp`n" | Out-File -FilePath $finalOutput -Encoding utf8
Get-ChildItem -Path $tmpDir -Filter "results_*.md" | Sort-Object Name | ForEach-Object {
    Get-Content $_.FullName | Out-File -FilePath $finalOutput -Append -Encoding utf8
    "`n---`n" | Out-File -FilePath $finalOutput -Append -Encoding utf8
}

Write-Host "Cleaning up..."
Remove-Item -Path $tmpDir -Recurse -Force

Write-Host "Done! Results saved to $finalOutput"
