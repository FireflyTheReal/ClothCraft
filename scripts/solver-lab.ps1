param(
    [Parameter(Mandatory=$true)][string]$Project,
    [string]$Output = 'output/solver-lab',
    [ValidateRange(1, 200)][int]$Iterations = 4,
    [ValidateRange(90, 3600)][int]$Steps = 120
)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $Output) { throw 'Choose a new output directory.' }
$source = (Resolve-Path -LiteralPath $Project).Path
$previousRayon = $env:RAYON_NUM_THREADS
try {
    $env:RAYON_NUM_THREADS = '2'
    cargo build --release --locked --no-default-features --example solver_compare -j 2
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    New-Item -ItemType Directory -Path $Output | Out-Null
    $exe = Join-Path $Output 'solver_compare.exe'
    Copy-Item -LiteralPath 'target/release/examples/solver_compare.exe' -Destination $exe
    $trackedSources = @('src/solver_lab/mod.rs', 'src/solver_lab/primal.rs', 'src/solver_lab/constraints.rs', 'src/solver_lab/dual.rs', 'examples/solver_compare.rs')
    $provenance = @{
        source = $source
        source_sha256 = (Get-FileHash -LiteralPath $source).Hash
        executable_sha256 = (Get-FileHash -LiteralPath $exe).Hash
        git_head = (git rev-parse HEAD)
        dirty_status = @(git status --short)
        rustc = (rustc --version)
        cpu = (Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
        iterations = $Iterations
        steps = $Steps
        threads = 2
        gpu = $false
        sources = @($trackedSources | ForEach-Object { @{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash} })
        load_note = 'Competing CPU load and power policy are not controlled.'
    }
    $provenance | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Output 'provenance.json')
    # First run creates an immutable portable copy. Every later run reads that copy.
    $canonical = $source
    foreach ($phase in @('quiet','pull')) {
        foreach ($backend in @('vbd','xpbd','avbd-finite')) {
            $destination = Join-Path $Output "$backend-$phase"
            $arguments = @('--project',$canonical,'--output',$destination,'--backend',$backend,'--steps',"$Steps",'--iterations',"$Iterations",'--export-lattice')
            if ($phase -eq 'pull') { $arguments += '--pull' }
            & $exe @arguments 1> (Join-Path $Output "$backend-$phase.stdout.log") 2> (Join-Path $Output "$backend-$phase.stderr.log")
            $code = $LASTEXITCODE
            if ($backend -eq 'vbd' -and $phase -eq 'quiet') {
                $canonical = Join-Path $destination 'source.json'
                if (!(Test-Path -LiteralPath $canonical)) { throw 'No canonical snapshot was created.' }
            }
            if ($code -ne 0) { Write-Warning "$backend/$phase failed; retained its report and continuing other contenders." }
        }
    }
} finally { $env:RAYON_NUM_THREADS = $previousRayon }
