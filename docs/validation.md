# Phase 1 validation — 14 October 2026

**Archive of the first CPU passes.** GPU restrictions and absent features below describe those historical runs. Avatar fitting, Delaunay and verified rendering were added later: see [first fitting](first-fit.md) and [meshing](meshing.md). That subsequent pass validated 40 CPU tests, warning-free CPU/desktop Clippy and separate native/GPU captures. Consult the current [handoff](handoff.md) for later results.

## Conditions

Windows, Rust/Cargo 1.97.1, AMD Ryzen 7 2700X Eight-Core Processor. Simulations below use the **no-desktop-feature** release binary, four Rayon threads, `dt = 1/60 s`, a 1×1 m pattern with 10×10 cells (121 vertices, 200 triangles), default parameters and 120 steps (two simulated seconds).

No GPU test, wgpu adapter, graphics window or CUDA program was launched. The user's GPU was reserved for other work. These are local timings on an unisolated machine, not a statistical study or CPU/GPU comparison. Current small-check policy uses two CPU jobs/threads; the four-thread commands below are historical evidence.

## Commands actually run

```powershell
cargo build --locked -j 4
cargo test --locked --no-default-features -j 4
cargo clippy --locked --all-targets -j 4 -- -D warnings
cargo fmt --all -- --check
.\scripts\bench-cpu.ps1 -Steps 120 -Resolution 10 -Threads 4
.\target\release\marvelous-cloth.exe --headless --project output\cpu-suite\sphere\project.json --steps 1 --output output\reload-check
cargo tree --locked --no-default-features -e normal --prefix none
```

The suite also compiles `cargo build --locked --release --no-default-features -j 4`. The no-desktop normal dependency tree contains no eframe, egui, winit or wgpu. Shader tests use only Naga's CPU parser/validator.

The desktop build was adapted to the current glam camera, egui panel and wgpu pipeline/depth APIs. A Clippy warning for automatic differentiation's product rule is explicitly allowed at that location with a justification; the final check passes with `-D warnings`.

## Passing tests: 17

Sixteen numerical/integration tests in `tests/numerics.rs` cover:

- Membrane gradients/Hessians in tension, compression and rest, bending and contact, compared with finite differences; Hessian symmetry.
- Force-free flat rest and integrated mass equal to density × area.
- Rigid-motion energy invariance, force transformation and internal force/torque balance.
- Warp/weft exchange under 90° grain rotation and softer compression response.
- Analytical implicit-Euler free fall with damping.
- Exact fixed points, objective decrease and convergence over 20 suspended steps.
- Sphere draping with active contact over 35 steps.
- Reported/reduced shallow initial penetration.
- State preservation after an invalid step; rejection of degenerate triangles.
- Distinction between exhausted Newton budget and convergence.
- JSON serialization, OBJ/UV export and invalid-parameter rejection.
- Nearly flat bending down to a `1e-10 m` position perturbation, positive energy without clamping.

One `tests/shader.rs` test validates WGSL syntax/semantics without a graphics device. Loading a CLI-produced JSON was also exercised with one converged headless step.

## Final measurements

Values copied from actual results into [phase1-results.json](phase1-results.json). The script can overwrite `output/cpu-suite/` on rerun; this copy retains that session's values.

| Scene | Converged steps | Mean CPU / step | Final RMS residual | Final maximum axial strain |
|---|---:|---:|---:|---:|
| Sphere, grain 0° | 120/120 | 31.330 ms | 3.418e-10 N | 2.160% |
| Suspended, 0° | 120/120 | 12.313 ms | 9.043e-12 N | 7.234% |
| Suspended, 45° | 120/120 | 16.721 ms | 2.560e-9 N | 5.909% |
| Suspended, 90° | 120/120 | 9.935 ms | 6.283e-7 N | 4.414% |

Final tolerance: `1e-6 N`. Displayed strain is the maximum `max(|lambda_warp-1|, |lambda_weft-1|)` over triangles, not a bound on every singular value of F. Timing differences between angles also reflect iteration counts and relaxation speeds; these are not comparisons between solvers.

For the sphere: maximum vertex penetration over the trajectory **0.750330 mm**, final penetration **0.178627 mm**, 53 contacting vertices at the end. Final energies: membrane 0.008915918 J, bending 0.005072804 J, contact 0.000114867 J. Estimated final element-array storage: 561,664 bytes, not total process memory.

Bending energy was algebraically reformulated using differences of unit normals to avoid negative values around `1e-18 J` from cancellation in `1-cos(theta)` at rest. Tests and all four scenes were rerun afterward; the table is from the corrected version.

Outputs include OBJ/UV, JSON projects, summaries and complete per-step metric series. The desktop binary was built at `target/debug/marvelous-cloth.exe`; the CPU-only binary at `target/release/marvelous-cloth.exe`.

## What remained unvalidated at that stage

The window, actual interaction and rendering on the GPU driver had not been visually tested. Compilation and Naga do not replace that check; it was deferred until GPU availability.

Tests proved neither nonintersection nor robustness under every large deformation. Contact then checked only vertices against a sphere; face/edge contact, self-collision, CCD, friction, layers and avatars were absent. Directional bending is simplified and parameters uncalibrated. No result for 10k/50k/200k vertices, GPU compute, VBD/AVBD/PD or IPC was claimed in this initial pass.

## Template and sewing increment — 14 October 2026

The multi-panel pipeline was implemented and checked on CPU only. Its 31 tests included the preceding 17, 11 template/sewing tests and three material-attachment tests. `scripts/check-cpu.ps1` passed formatting, tests, warning-free Clippy and both example validations.

After integration, `cargo build --locked -j 2`, `cargo clippy --locked --all-targets -j 2 -- -D warnings` (default desktop features) and `cargo fmt --all -- --check` also passed. These compile/analyze the renderer without running it. GitHub CI was prepared, not remotely executed.

New tests cover analytical constant/linear gap integration on different meshes; gradient/Hessian finite differences; symmetry and positive quadratic form; force/torque balance; rigid invariance; boundary reversal; partial intervals; notches; explicit easing; invalid inputs; shared vertices; independent mass/grain; serialization; multi-material solving compared with independent solves; and transactional failure for an invalid compiled seam. Guides are checked on a 90° folded surface and after changing resolution.

Actually executed command, release binary without desktop:

```powershell
.\target\release\cloth-template.exe simulate examples/templates/two-panel.json --steps 30 --threads 2 --output output/template-simulation
```

| Measurement | Result |
|---|---:|
| Panels / vertices / triangles | 2 / 60 / 80 |
| Panel resolutions | 3×4 and 4×7 cells |
| Seams / Gauss samples | 1 / 20 |
| Converged steps | 30 / 30 |
| Physical time | 0.5 s |
| Initial maximum gap | 96.4197 mm |
| Final maximum gap | 1.7740 mm |
| Final RMS gap | 0.6255 mm |
| Final RMS residual | 9.8055e-9 N |
| Final tolerance | 2.8534e-6 N |
| Final sewing energy | 5.8684e-4 J |
| Mean CPU / step | 29.75 ms |

Full values: [template-results.json](template-results.json). Timing is indicative on the two-thread workstation, not a capacity benchmark. Both panels are explicitly placed in 3D; the left has two fixed corners. `k=5000 N/m²` is an uncalibrated demonstration value. Tolerance includes the step-initial residual's relative term.

`local-ease.json` was validated/exported with an unequal-local-length warning; no physical-gather or validated-simulation claim is made for it. Generated SVG parsed as XML; all 80 simulation fiber frames were valid. No window or GPU adapter was created.

Newton convergence establishes neither industrial seam fidelity nor nonintersection: the connection is a penalty, the panels in this example have no mutual contact, and seam thickness/folds/stitches are not modeled. The GUI was still single-panel at that historical stage; the new format was available through the library/CLI. Later workspace capabilities are documented separately.
