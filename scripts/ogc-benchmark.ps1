param(
    [switch]$Gpu,
    [switch]$UiSmoke,
    [ValidateRange(1, 1000)][int]$Steps = 120,
    [ValidateRange(8, 60)][double]$MeshMm = 30
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspace
try {
    $build = @('build', '--release', '--locked', '-j', '2', '--bin', 'cloth-fit')
    if (-not ($Gpu -or $UiSmoke)) { $build += '--no-default-features' }
    cargo @build
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    foreach ($kind in @('tshirt', 'dress')) {
        $arguments = @('--garment', $kind, '--steps', $Steps, '--mesh-mm', $MeshMm, '--output', "output/ogc-benchmark/$kind")
        if ($Gpu) { $arguments += @('--gpu-sim', '--gpu-benchmark') }
        if ($UiSmoke) { $arguments += '--ui-smoke' }
        & .\target\release\cloth-fit.exe @arguments
        if ($LASTEXITCODE -ne 0) { throw "OGC run failed: $kind" }
    }
} finally { Pop-Location }
