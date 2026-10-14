# First source publication — validation

2026-10-14, local Windows workstation. Machine-readable results: [publication-validation.json](publication-validation.json). These checks cover the first authoring slice and integration with the existing physics; they are not an industrial fitting certification.

- `scripts/check-cpu.ps1`: **63 CPU tests pass**, formatting, CPU Clippy and four example-template validations pass. No GPU initialized by this suite.
- `cargo clippy --locked --all-targets -j 2 -- -D warnings`: pass, including desktop code.
- `cargo build --release --locked -j 2 --bin cloth-fit`: pass.
- `cloth-fit --steps 0 --ui-authoring-smoke --output output/authoring-release`: **20 native pointer actions**, polygon creation, translation and rotation, duplication, sewing, observed undo/redo, export/reload/compile pass. [Actual capture](images/authoring.png).
- `cloth-fit --project output/turbo-repro/tshirt/fitting-project.json --steps 0 --ui-perf-smoke --output output/authoring-release-perf`: **59.99 UI frames/s, 59.99 physical steps/s, 0.99986× physical/wall time**, after warm-up. Frame interval P95: 17.58 ms. [Actual capture](images/atelier.png).
- Same saved T-shirt with `--ui-interaction-smoke --output output/authoring-release-grab`: actual pointer pick and accepted grab step pass; maximum displacement 13.39 mm, including ongoing dynamics.

The last two runs use the 25 mm, 2,454-vertex / 4,570-triangle T-shirt at 6 s from the earlier [Turbo reproduction](turbo-perf.md), eight iterations and a 1/60 s timestep on the RTX 3090 workstation. Generate that starting project with `scripts/turbo-perf.ps1 -Steps 360 -MeshMm 25` before reproducing the commands. Output directories and old executables are not committed. No competing build ran during these window measurements.

The authoring smoke uses a hardware-rendered window but does not step cloth physics. The performance/grab smokes also run hybrid GPU physics. All three are explicit opt-ins and absent from ordinary CI. GitHub Actions status must be checked separately on the published commit.
