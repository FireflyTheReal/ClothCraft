param(
    [Parameter(Mandatory=$true)][string]$Project,
    [string]$Output = 'output/solver-consistency',
    [ValidateRange(90, 3600)][int]$Steps = 120
)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $Output) { throw 'Choose a new evidence directory.' }
$source = (Resolve-Path -LiteralPath $Project).Path
$previousRayon = $env:RAYON_NUM_THREADS
try {
    $env:RAYON_NUM_THREADS = '2'
    cargo build --release --locked --no-default-features --example solver_compare -j 2
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    New-Item -ItemType Directory -Path $Output | Out-Null
    $exe = Join-Path $Output 'solver_compare.exe'
    Copy-Item -LiteralPath 'target/release/examples/solver_compare.exe' -Destination $exe
    $files = @('src/solver_lab/mod.rs','src/solver_lab/primal.rs','src/solver_lab/constraints.rs','src/solver_lab/dual.rs','examples/solver_compare.rs','tests/solver_lab.rs','scripts/solver-consistency.ps1')
    foreach ($file in $files) {
        $copy = Join-Path (Join-Path $Output 'source') $file
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $copy) | Out-Null
        Copy-Item -LiteralPath $file -Destination $copy
    }
    @{
        source=$source; source_sha256=(Get-FileHash -LiteralPath $source).Hash
        executable_sha256=(Get-FileHash -LiteralPath $exe).Hash
        git_head=(git rev-parse HEAD); dirty_status=@(git status --short)
        rustc=(rustc --version); threads=2; gpu=$false
        cpu=(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
        source_hashes=@($files | ForEach-Object { @{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash} })
        load_note='Uncontrolled background load. Convergence tracing includes diagnostic cost; no speed ranking.'
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Output 'provenance.json')
    $canonical = $source
    foreach ($backend in @('xpbd','avbd-finite','xpbd-primal','avbd-target')) {
        foreach ($iterations in @(4,16,64)) {
            $name = "single-$backend-$iterations"
            $destination = Join-Path $Output $name
            & $exe --project $canonical --output $destination --backend $backend --steps 1 --iterations $iterations --convergence --audit-every-step 1> (Join-Path $Output "$name.stdout.log") 2> (Join-Path $Output "$name.stderr.log")
            if ($LASTEXITCODE -ne 0) { Write-Warning "$name failed; evidence retained." }
            if ($canonical -eq $source) { $canonical = Join-Path $destination 'source.json' }
            Write-Output "Completed $name"
        }
    }
    $dt = (Get-Content -LiteralPath (Join-Path $Output 'single-xpbd-4/protocol.json') -Raw | ConvertFrom-Json).settings.dt
    foreach ($phase in @('quiet','pull')) {
        foreach ($backend in @('vbd','xpbd-primal','avbd-target','xpbd')) {
            $factor = if ($backend -eq 'xpbd') { 4 } else { 1 }
            $iterations = if ($factor -eq 4) { 1 } else { 4 }
            $name = if ($factor -eq 4) { "xpbd-small-$phase" } else { "$backend-$phase" }
            $arguments = @('--project',$canonical,'--output',(Join-Path $Output $name),'--backend',$backend,'--steps',"$($Steps*$factor)",'--iterations',"$iterations",'--dt',($dt/$factor).ToString('R',[Globalization.CultureInfo]::InvariantCulture),'--pull-start',"$(30*$factor)",'--pull-steps',"$(30*$factor)",'--hold-steps',"$(15*$factor)",'--audit-every-step')
            if ($phase -eq 'pull') { $arguments += '--pull' }
            & $exe @arguments 1> (Join-Path $Output "$name.stdout.log") 2> (Join-Path $Output "$name.stderr.log")
            if ($LASTEXITCODE -ne 0) { Write-Warning "$name failed; evidence retained." }
            Write-Output "Completed $name"
        }
    }
} finally { $env:RAYON_NUM_THREADS = $previousRayon }
