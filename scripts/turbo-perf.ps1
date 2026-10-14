param(
    [ValidateRange(1, 3600)][int]$Steps = 360,
    [ValidateRange(8, 60)][double]$MeshMm = 25,
    [switch]$UiPerf
)
# Explicit GPU benchmark: never invoked by check-cpu.ps1 or ordinary tests.
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspace
try {
    cargo build --release --locked -j 2 --bin cloth-fit
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    foreach ($garment in @('tshirt', 'dress')) {
        $destination = "output/turbo-repro/$garment"
        & .\target\release\cloth-fit.exe --garment $garment --mesh-mm $MeshMm --gpu-sim --iterations 16 --dt '0.016666666666666666' --steps $Steps --output $destination
        if ($LASTEXITCODE -ne 0) { throw "GPU simulation failed: $garment" }
        if ($UiPerf) {
            & .\target\release\cloth-fit.exe --project "$destination/fitting-project.json" --steps 0 --ui-perf-smoke --output "$destination/ui"
            if ($LASTEXITCODE -ne 0) { throw "Window/physics cadence check failed: $garment" }
        }
    }
} finally { Pop-Location }
