param(
    [Parameter(Mandatory=$true)][string]$Project,
    [switch]$Gpu,
    [switch]$Compare,
    [ValidateRange(1,10000)][int]$Steps = 1800,
    [string]$Output
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $workspace
try {
    $inputProject = (Resolve-Path -LiteralPath $Project).Path
    if (!$Output) { $Output = Join-Path $workspace ('output/drape-checkpoint/' + [guid]::NewGuid().ToString('N')) }
    if (Test-Path -LiteralPath $Output) { throw 'Use a new output directory; benchmark evidence is never overwritten.' }
    $evidence = (New-Item -ItemType Directory -Path $Output).FullName
    & cargo build --release --locked -j 2 --bin cloth-fit
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    $binary = Join-Path $workspace 'target/release/cloth-fit.exe'
    Copy-Item -LiteralPath $binary -Destination "$evidence/runner.exe"
    $binary = Join-Path $evidence 'runner.exe'
    @{ source = $inputProject; source_sha256 = (Get-FileHash -LiteralPath $inputProject).Hash; binary_sha256 = (Get-FileHash -LiteralPath $binary).Hash; steps = $Steps; gpu = [bool]$Gpu; cpu_workers = 2 } | ConvertTo-Json | Set-Content -LiteralPath "$evidence/manifest.json" -Encoding utf8
    # A zero-step CPU export copies a validated document and portable avatar.
    & $binary --project $inputProject --steps 0 --threads 2 --output "$evidence/source" 1> "$evidence/source.log" 2> "$evidence/source-errors.log"
    if ($LASTEXITCODE -ne 0) { throw 'Source snapshot failed' }
    $backend = @()
    if ($Gpu) { $backend += '--gpu-sim' }
    & $binary --project "$evidence/source/fitting-project.json" --steps $Steps --threads 2 @backend --output "$evidence/drape" 1> "$evidence/drape.log" 2> "$evidence/drape-progress.log"
    if ($LASTEXITCODE -ne 0) { throw 'Drape failed' }
    if ($Compare) {
        foreach ($trial in @(@('rest16',600,16,'0.016666666666666666'), @('rest64',600,64,'0.016666666666666666'), @('substeps2',1200,8,'0.008333333333333333'))) {
            & $binary --project "$evidence/drape/fitting-project.json" --steps $trial[1] --iterations $trial[2] --dt $trial[3] --threads 2 @backend --output "$evidence/$($trial[0])" 1> "$evidence/$($trial[0]).log" 2> "$evidence/$($trial[0])-progress.log"
            if ($LASTEXITCODE -ne 0) { throw "Comparison $($trial[0]) failed" }
        }
    }
    & python scripts/summarize-drape.py $evidence
    if ($LASTEXITCODE -ne 0) { throw 'Summary failed' }
    Write-Output "Checkpoint with velocities: $evidence/drape/fitting-project.json"
} finally { Pop-Location }
