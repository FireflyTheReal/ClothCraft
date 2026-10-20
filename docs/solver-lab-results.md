# First dress comparison — 19 October 2026

Follow-up: [consistency and convergence experiments](solver-consistency-results.md)
retain these original prototypes as controls and test full-residual coupling,
target-complete finite AVBD and smaller XPBD timesteps. The table below is the
historical first screening, not the latest implementation's timing baseline.

This is a short **candidate screening**, not a claim that the dress settles without jitter. No candidate replaces the editor's solver. Implementation scope and commands are in [solver-lab.md](solver-lab.md).

## Input and protocol

- User's saved dress: SHA-256 `E542E066F4BF662149F8603BAB47913A60498915689386F0BD51503E4D7AACCB`. The user's file was not modified.
- 5,126 vertices, 9,546 cloth triangles, six panels, seven seams; avatar: 26,756 triangles. One panel is disconnected. Per-panel speed is retained instead of interpreting global free-fall speed as jitter.
- All ten Rust runs start from the same serialized state, SHA-256 `18174FE9D1AFDAB39B95BD9DDE0826B5E02874319F51776A1BBCE179976F87DF`. Saved time is approximately 49.45 s. Backend caches cold-start. This is not a force-balanced equilibrium checkpoint.
- Each run advances 120 steps at 1/60 s: two seconds of physical time. Quiet continuation and a separate matched interaction: 30 quiet steps, 30 steps pulling 40 mm in Z, 15 holding, 45 releasing. Same triangle, initial world anchor and 1,000 N/m barycentric grab across Rust adapters.
- Ten warm-up steps excluded from timing summaries. Full accepted step costs include CPU work and, for GPU, transfers/wait/readback. Fresh CPU geometry audits and report writing are outside timings. These are headless simulation timings, not viewport FPS.
- Ryzen 7 2700X, two Rayon threads; release Rust 1.97.1. GPU control: RTX 3090, Vulkan, NVIDIA driver 616.56. Before GPU runs: 9% utilization, 1,594 MiB allocated, P8. Competing background load and power policy were not controlled. Each row is one run, not a repeated statistical experiment.
- Local raw evidence and archived executables: `output/solver-lab-20261019/`. Public summary: [solver-lab-validation.json](solver-lab-validation.json). Private geometry/assets remain in ignored output folders.

## CPU contenders, four sweeps per step

Equal sweep counts do not mean equal arithmetic or convergence. The AVBD adaptation ramps effective stiffness; the XPBD hybrid clips prediction and constraint steps. This is a quality/cost comparison, not an equal-integrator speed ranking.

| Backend | Quiet mean / P95 ms | Pull/release mean / P95 ms | Final seam gap, quiet / release | Final stretch P95, quiet / release |
| --- | ---: | ---: | ---: | ---: |
| Existing VBD | 261.0 / 282.3 | 263.4 / 284.9 | 13.6 / 13.5 mm | 19.0 / 19.1% |
| XPBD + OGC research hybrid | 398.7 / 419.2 | 396.3 / 429.3 | 475.4 / 509.4 mm | 346.0 / 351.4% |
| AVBD finite-ramp adaptation | 1,144.7 / 1,206.3 | 1,135.5 / 1,179.3 | 97.9 / 103.2 mm | 67.8 / 69.3% |

All six complete their requested 120 steps with finite states and pass the final surface-intersection check under the existing local seam exclusions. That does **not** qualify the two new material responses: both fail the deformation/sewing quality gate at this budget. Collision separation alone is insufficient.

XPBD's poor result needs investigation of primal consistency under OGC clipping, contact feature multipliers and convergence/substeps. It is not evidence that XPBD in general is slow or unsuitable. AVBD's result belongs to this sequential f64, finite-ramp adaptation and its explicit tuning; it is not a performance result for the authors' GPU implementation. No new GPU kernel was introduced.

## Existing hybrid GPU VBD control

| Sweeps | Quiet mean / P95 ms | Pull/release mean / P95 ms | Final seam gap, quiet / release |
| --- | ---: | ---: | ---: |
| 4 | 12.67 / 18.17 | 11.92 / 15.74 | 11.7 / 11.6 mm |
| 16 | 15.46 / 18.89 | 15.50 / 20.59 | 21.9 / 15.1 mm |

All four complete 120 steps and pass final intersection checks. At 16 sweeps the mean is below the 16.67 ms budget, but P95 is above it: **this does not establish sustained 60 steps/s without spikes**. Different finite-iteration trajectories and cold predictor transients mean that the smaller final seam gap at four sweeps is not proof of higher accuracy. This short experiment does not select a production iteration budget or establish absence of jitter.

## Actual Box3D lattice: rejected

The harness links Box3D commit `efca0c184d40fd6a82016983f1601c4d23965867`, compiled with MSVC 19.50. It creates 5,126 particle bodies and 15,365 distance springs, with two worker threads, four substeps, zero surface friction and sleep disabled. It uses the same avatar and compiled masses. Its edge springs, pairwise sewing approximation, absence of bending, float precision and sphere contact are **different** from the cloth continuum model.

The stock 512-triangle contact buffer saturated. The raw stock quiet/pull runs were retained but their timings are excluded from a valid comparison. With the capacity explicitly increased to 4,096, the harness aborts at step 26 on another saturation, retaining the preceding 26 states. The capacity is a CMake option; the harness now treats that warning as a failed run rather than silently continuing. Native distance-joint rest lengths also clamp to at least 5 mm at the stock length scale.

The decisive result is independent of that later saturation: **the original cloth surface already intersects itself after the first accepted Box3D step**. The independent ClothCraft audit detects it at trace index 0. At index 25 it still reports intersections, zero separation, 154.8 mm maximum seam gap and 279.2% P95 extension. The valid-capacity test never reaches its scheduled pull, so no successful native pull/release result is claimed.

This does not discredit Box3D's rigid-body solver. It rejects the specific idea that vertex spheres plus distance joints provide a ready cloth replacement. Triangle/edge collision coverage and a suitable shell/sewing model would be required before meaningful competition. Expanding a contact buffer cannot fill the geometric holes between particle spheres.

## What to try next

Validation completed: 170 CPU tests, CPU Clippy across all targets with warnings denied, rustfmt and six template validations pass through `scripts/check-cpu.ps1`. The desktop-enabled comparison runner also passes Clippy. Native CMake compilation passes. After final input-validation cleanup, the native harness reproduces the same 26-state trace byte for byte and returns the expected failure code on capacity saturation. No commit, push or editor executable replacement was performed.

1. Keep the current VBD/OGC path as a control, not a permanent winner. Extend the test harness to long release/settling windows and converged reference forces before optimizing new kernels.
2. Repair XPBD's inertial residual when conservative contact limits clip prediction. Test scalar analytic problems, off-axis coupled constraints and constrained pull/release before revisiting this dress.
3. For AVBD, validate stiffness continuation against the full material target and compare converged responses. Hard-constraint/dual support must be implemented and tested explicitly; it is not present merely because the adapter is named after AVBD.
4. Reuse rigid-body contact engineering where appropriate, but treat a Box3D cloth coupling as a distinct research project with surface contacts. Preserve the failed lattice case.
5. Only then compare optimized CPU/GPU implementations at matched material/contact quality. Do not hide residual forces with sleep, velocity thresholds or garment-specific rules.
