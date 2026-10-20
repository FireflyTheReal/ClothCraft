param(
    [switch]$Gpu,
    [string]$Project,
    [string]$Panel,
    [double[]]$Point,
    [UInt64]$Seed = 288505435208,
    [ValidateRange(1, 100000)][int]$Cases = 512,
    [ValidateRange(211, 10000)][int]$Steps = 360,
    [string]$Output
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$previousRayon = $env:RAYON_NUM_THREADS
$previousSeed = $env:CLOTHCRAFT_ZOO_SEED
$previousCases = $env:CLOTHCRAFT_ZOO_CASES
Push-Location -LiteralPath $workspace
try {
    if ($Project) {
        $Project = (Resolve-Path -LiteralPath $Project).Path
        if ($Gpu -and (!$Panel -or $Point.Count -ne 2)) {
            throw 'A project replay needs -Panel and -Point u,v in material metres.'
        }
    }
    if (!$Output) { $Output = Join-Path $workspace ('output/physics-lab/' + [guid]::NewGuid().ToString('N')) }
    if (Test-Path -LiteralPath $Output) { throw 'Choose a new output directory; laboratory evidence is never overwritten.' }
    $labOutput = (New-Item -ItemType Directory -Path $Output).FullName
    $env:RAYON_NUM_THREADS = '2'
    $env:CLOTHCRAFT_ZOO_SEED = $Seed.ToString()
    $env:CLOTHCRAFT_ZOO_CASES = $Cases.ToString()
    $manifest = @{ seed = $Seed; cases = $Cases; steps = $Steps; gpu = [bool]$Gpu; project = $Project; panel = $Panel; material_point_m = $Point; cpu_workers = 2 }
    if ($Project) { $manifest.project_sha256 = (Get-FileHash -LiteralPath $Project -Algorithm SHA256).Hash }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $labOutput 'manifest.json') -Encoding utf8

    & cargo test --release --locked --no-default-features -j 2 --test directional_zoo -- --test-threads=2 --nocapture 2>&1 | Tee-Object -FilePath (Join-Path $labOutput 'geometry.log')
    if ($LASTEXITCODE -ne 0) { throw 'CPU geometry zoo failed; seed and output are preserved.' }
    if ($Gpu) {
        & cargo build --release --locked -j 2 --example seam_block_probe
        if ($LASTEXITCODE -ne 0) { throw 'Headless replay build failed.' }
        $manifest.replay_binary_sha256 = (Get-FileHash -LiteralPath 'target/release/examples/seam_block_probe.exe' -Algorithm SHA256).Hash
        $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $labOutput 'manifest.json') -Encoding utf8
        & cargo test --release --locked -j 2 --test gpu_directional -- --ignored --test-threads=1 --nocapture 2>&1 | Tee-Object -FilePath (Join-Path $labOutput 'gpu-regressions.log')
        if ($LASTEXITCODE -ne 0) { throw 'GPU motion regression failed.' }
        $common = @('--gpu', '--baseline-only', '--steps', $Steps.ToString())
        if ($Project) {
            $common += @('--project', $Project, '--panel', $Panel, '--point',
                $Point[0].ToString('R', [cultureinfo]::InvariantCulture),
                $Point[1].ToString('R', [cultureinfo]::InvariantCulture))
        } else { $common += @('--nonmatching', '--ease-ratio', '1.25') }
        $baseline = Join-Path $labOutput 'isotropic'
        & target/release/examples/seam_block_probe.exe @common --output $baseline 2>&1 | Tee-Object -FilePath (Join-Path $labOutput 'isotropic.log')
        if ($LASTEXITCODE -ne 0) { throw 'Isotropic replay failed.' }
        $controls = Join-Path $baseline 'controls.jsonl'
        & target/release/examples/seam_block_probe.exe @common --directional-bounds --controls-from $controls --output (Join-Path $labOutput 'directional') 2>&1 | Tee-Object -FilePath (Join-Path $labOutput 'directional.log')
        if ($LASTEXITCODE -ne 0) { throw 'Directional replay failed.' }
        & target/release/examples/seam_block_probe.exe @common --directional-bounds --controls-from $controls --check-gpu-contacts --output (Join-Path $labOutput 'oracle') 2>&1 | Tee-Object -FilePath (Join-Path $labOutput 'oracle.log')
        if ($LASTEXITCODE -ne 0) { throw 'Independent CPU contact oracle failed.' }
        & python scripts/summarize-physics-lab.py $labOutput
        if ($LASTEXITCODE -ne 0) { throw 'Comparison summary failed.' }
    }
    Write-Output "Headless laboratory completed: $labOutput"
} finally {
    $env:RAYON_NUM_THREADS = $previousRayon
    $env:CLOTHCRAFT_ZOO_SEED = $previousSeed
    $env:CLOTHCRAFT_ZOO_CASES = $previousCases
    Pop-Location
}
