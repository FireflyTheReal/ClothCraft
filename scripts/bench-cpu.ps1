param(
    [ValidateRange(1, 10000)][int]$Steps = 120,
    [ValidateRange(2, 32)][int]$Resolution = 10,
    [ValidateRange(1, 64)][int]$Threads = 4
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspace
try {
    cargo build --locked --release --no-default-features -j 4
    if ($LASTEXITCODE -ne 0) { throw 'CPU build failed' }
    $cases = @(
        @{ Name = 'sphere'; Scene = 'sphere'; Grain = 0 },
        @{ Name = 'hanging-0'; Scene = 'hanging'; Grain = 0 },
        @{ Name = 'hanging-45'; Scene = 'hanging'; Grain = 45 },
        @{ Name = 'hanging-90'; Scene = 'hanging'; Grain = 90 }
    )
    $rows = foreach ($case in $cases) {
        $destination = Join-Path 'output/cpu-suite' $case.Name
        & '.\target\release\marvelous-cloth.exe' --headless --scene $case.Scene --grain $case.Grain --resolution $Resolution --steps $Steps --threads $Threads --output $destination | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "CPU run failed: $($case.Name)" }
        $summary = Get-Content -LiteralPath (Join-Path $destination 'summary.json') -Raw | ConvertFrom-Json
        [PSCustomObject]@{
            scene = $case.Name
            vertices = $summary.last_step.vertices
            steps = $summary.steps
            converged_steps = $summary.converged_steps
            mean_step_ms = $summary.mean_step_ms
            last_residual_n = $summary.last_step.residual_rms_n
            last_strain = $summary.last_step.max_strain
            max_vertex_penetration_m = $summary.max_vertex_penetration_m
        }
    }
    $rows | ConvertTo-Json | Set-Content -LiteralPath 'output/cpu-suite/suite.json' -Encoding utf8
    $rows | Format-Table
} finally {
    Pop-Location
}
