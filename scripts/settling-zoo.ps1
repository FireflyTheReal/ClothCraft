param(
    [Parameter(Mandatory = $true)][string]$Output,
    [string[]]$Cases = @(),
    [int]$Seed = 0,
    [int]$Resolution = 3,
    [int]$Iterations = 16,
    [double]$SettleSeconds = 20,
    [double]$ContinuationSeconds = 10,
    [switch]$UnshiftedSpd,
    [int]$Backtracks = 10,
    [string]$ControlsFrom = ''
)
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    if (Test-Path -LiteralPath $Output) { throw "Evidence directory already exists: $Output" }
    cargo build --release --locked --no-default-features -j 2 --example settling_zoo
    if ($LASTEXITCODE -ne 0) { throw 'CPU zoo build failed' }
    $zooArguments = @('--output', $Output, '--seed', $Seed, '--resolution', $Resolution,
        '--iterations', $Iterations, '--settle-seconds', $SettleSeconds,
        '--continuation-seconds', $ContinuationSeconds, '--backtracks', $Backtracks)
    if ($Cases.Count -gt 0) { $zooArguments += @('--cases', ($Cases -join ',')) }
    if ($UnshiftedSpd) { $zooArguments += '--unshifted-spd' }
    if ($ControlsFrom) { $zooArguments += @('--controls-from', $ControlsFrom) }
    & .\target\release\examples\settling_zoo.exe @zooArguments
    $zooExitCode = $LASTEXITCODE
    if (Test-Path -LiteralPath $Output) {
        Copy-Item -LiteralPath 'target\release\examples\settling_zoo.exe' -Destination (Join-Path $Output 'runner.exe')
        $zooSources = @('examples/settling_zoo.rs', 'src/vbd.rs', 'src/vbd/local_step.rs', 'Cargo.lock')
        @{
            binary = Get-FileHash (Join-Path $Output 'runner.exe')
            sources = @($zooSources | ForEach-Object { Get-FileHash -LiteralPath $_ })
            git_head = (git rev-parse HEAD)
            git_status = @(git status --short)
            exit_code = $zooExitCode
        } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $Output 'provenance.json')
    }
    if ($zooExitCode -ne 0) { throw "CPU zoo failed ($zooExitCode); retain its partial evidence" }
} finally {
    Pop-Location
}
