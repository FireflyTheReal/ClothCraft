param(
    [ValidateRange(0, 100)][int]$Steps = 5,
    [ValidateRange(8, 60)][double]$MeshMm = 25,
    [switch]$Gpu,
    [switch]$UiSmoke
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspace
try {
    cargo build --locked --release --no-default-features -j 2 --bin cloth-fit
    if ($LASTEXITCODE -ne 0) { throw 'CPU build failed' }
    $failed = $false
    foreach ($kind in @('tshirt', 'dress')) {
        & .\target\release\cloth-fit.exe --reference --garment $kind --mesh-mm $MeshMm --steps $Steps --threads 2 --output "output/first-fit/$kind"
        if ($LASTEXITCODE -ne 0) { $failed = $true }
    }
    if ($Gpu -or $UiSmoke) {
        cargo build --locked -j 2 --bin cloth-fit
        if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed' }
        foreach ($kind in @('tshirt', 'dress')) {
            $arguments = @('--reference', '--project', "output/first-fit/$kind/fitting-project.json", '--steps', '0', '--gpu-benchmark', '--output', "output/first-fit/$kind/gpu")
            if ($UiSmoke) { $arguments += '--ui-smoke' }
            & .\target\debug\cloth-fit.exe @arguments
            if ($LASTEXITCODE -ne 0) { throw "GPU check failed: $kind" }
        }
    }
    if ($failed) { throw 'At least one CPU fitting failed; inspect each run.json and the last accepted state' }
} finally {
    Pop-Location
}
