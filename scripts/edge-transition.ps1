param(
    [string]$Project,
    [ValidateRange(120, 3600)][int]$Steps = 120,
    [ValidateRange(1, 200)][int]$Iterations = 16,
    [string]$Output = ("output/edge-transition-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$previousRayon = $env:RAYON_NUM_THREADS
Push-Location -LiteralPath $workspace
try {
    $env:RAYON_NUM_THREADS = '2'
    if (Test-Path -LiteralPath $Output) { throw 'Choose a new output directory' }
    cargo test --locked --no-default-features -j 2 --test edge_transition -- --test-threads=2
    if ($LASTEXITCODE -ne 0) { throw 'CPU transition tests failed' }
    cargo build --release --locked --no-default-features -j 2 --example solver_compare --example edge_transition_probe
    if ($LASTEXITCODE -ne 0) { throw 'CPU laboratory build failed' }
    New-Item -ItemType Directory -Path $Output | Out-Null
    Copy-Item -LiteralPath 'target/release/examples/solver_compare.exe' -Destination "$Output/solver_compare.exe"
    Copy-Item -LiteralPath 'target/release/examples/edge_transition_probe.exe' -Destination "$Output/edge_transition_probe.exe"
    & "$Output/edge_transition_probe.exe" --output "$Output/synthetic.json"
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic probe failed' }
    if ($Project) {
        & "$Output/solver_compare.exe" --project $Project --output "$Output/capture" --backend xpbd-primal --steps 1 --iterations 6 --convergence --audit-every-step > "$Output/capture.log"
        if ($LASTEXITCODE -ne 0) { throw 'Transition capture failed' }
        if (Test-Path -LiteralPath "$Output/capture/refresh-00000.json") {
            $capture = Get-Content -Raw -LiteralPath "$Output/capture/refresh-00000.json" | ConvertFrom-Json
            if ($capture.largest_feature_changes | Where-Object { $_.kind -eq 'EdgeEdge' }) {
                & "$Output/edge_transition_probe.exe" --project $Project --capture "$Output/capture/refresh-00000.json" --output "$Output/dress-transition.json"
                if ($LASTEXITCODE -ne 0) { throw 'EE geometry probe failed; inspect the capture' }
            } else {
                Write-Host 'No EE event in this capture; continuing the VBD comparisons.'
            }
        }
        foreach ($mode in @('baseline', 'interior')) {
            foreach ($phase in @('quiet', 'pull')) {
                $runArgs = @('--project', $Project, '--output', "$Output/vbd-$mode-$phase", '--backend', 'vbd', '--steps', $Steps, '--iterations', $Iterations, '--audit-every-step')
                if ($mode -eq 'interior') { $runArgs += '--interior-edges' }
                if ($phase -eq 'pull') { $runArgs += '--pull' }
                & "$Output/solver_compare.exe" @runArgs > "$Output/vbd-$mode-$phase.log"
                if ($LASTEXITCODE -ne 0) { throw "Replay failed: $mode $phase" }
                $summary = Get-Content -Raw -LiteralPath "$Output/vbd-$mode-$phase/summary.json" | ConvertFrom-Json
                if ($summary.failure -or $summary.accepted_steps -ne $Steps) { throw "Incomplete replay: $mode $phase" }
            }
        }
    }
    python scripts/summarize-edge-transition.py $Output
    if ($LASTEXITCODE -ne 0) { throw 'Summary failed' }
} finally {
    $env:RAYON_NUM_THREADS = $previousRayon
    Pop-Location
}
