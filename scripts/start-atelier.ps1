param([string]$ProjectPath = '')
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspace
try {
    cargo build --release --locked -j 2 --bin cloth-fit
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    $arguments = @('--steps', '0', '--view', '--output', 'output/atelier')
    if ($ProjectPath) { $arguments += @('--project', $ProjectPath) }
    & .\target\release\cloth-fit.exe @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Atelier failed; inspect the diagnostic above' }
} finally { Pop-Location }
