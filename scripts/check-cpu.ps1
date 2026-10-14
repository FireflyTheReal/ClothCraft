param([ValidateRange(1, 8)][int]$Jobs = 2)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$previousRayon = $env:RAYON_NUM_THREADS
Push-Location -LiteralPath $workspace
try {
    $env:RAYON_NUM_THREADS = '2'
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting failed' }
    cargo test --locked --no-default-features -j $Jobs -- --test-threads=2
    if ($LASTEXITCODE -ne 0) { throw 'CPU tests failed' }
    cargo clippy --locked --no-default-features --all-targets -j $Jobs -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'CPU clippy failed' }
    foreach ($name in @('two-panel', 'local-ease', 'tshirt', 'dress', 'curved-panels')) {
        cargo run --locked --no-default-features -j $Jobs --bin cloth-template -- validate "examples/templates/$name.json"
        if ($LASTEXITCODE -ne 0) { throw "Template validation failed: $name" }
    }
} finally {
    $env:RAYON_NUM_THREADS = $previousRayon
    Pop-Location
}
