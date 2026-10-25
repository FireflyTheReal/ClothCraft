# Bring a better cloth solver

ClothCraft should outlive any one physics algorithm. Keep the garment model, editing tools, interoperability and evaluation useful while experimenting with different solvers. Humans and AI agents are invited to build contenders: faster convergence, better contact, friction, material accuracy, or a better balance of all four.

## Existing boundary — and what is still missing

Update: [the solver lab](solver-lab.md) now contains concrete CPU adapters and a separate native Box3D harness. The [consistency study](solver-consistency.md) adds coupled optimization variants and quality-first diagnostics. The distinction below still applies to production: there is no universal plugin ABI or editor hot-swap selector, and research candidates must pass quality and contact gates before promotion.

`GarmentTemplate` compiles into `CompiledGarment`; `State` stores positions, velocities and physical time. These CPU types do not depend on the window. `ReferenceSolver`, `VbdSolver` and `GpuVbd` consume that model today, with separate adapters in the CLI/worker. **There is not yet a common backend trait, runtime plugin ABI or hot-swap selector.** Newton's contact model differs from OGC; do not present them as physically equivalent options.

The next backend interface must describe construction from a compiled model/collider, accepted stepping, diagnostic output, reset and teardown. Declare capabilities such as static/animated avatars, contact type, friction, precision and device needs. A topology/material change invalidates caches; switching algorithms requires an explicit state-transfer policy. Do not create an empty trait hierarchy: first implement and test two working adapters. Keep the historical `ReferenceSolver::step` API.

## Benchmark contract

1. Pin the source commit, build profile/compiler, hardware/backend/driver, thread count, scene hash and initial state. Record competing GPU load and power settings when known.
2. Hold geometry, material laws, contacts, timestep, iteration/convergence budget and simulated duration constant for a speed-only claim. If any change, report a quality/cost comparison instead.
3. Measure end-to-end wall time per accepted step, median/P95/max, warm-up and sample count. Break out contact, transfers and compute. A shader timestamp is not total simulation time; steps/s is not viewport FPS.
4. Report physical progress per wall second and actual UI cadence. Pausing physical time or dropping required steps must not count as realtime.
5. Check finite states, rest preservation, seam gap, stretch/strain, energy or residual as appropriate, contact violations and failed/retried steps. Compare forces to the f64 reference; add convergence and analytic tests appropriate to a changed law.
6. Include cold assembly and settled motion. Extend the scene suite with dense garments, folds, layers, motion and interactions before generalizing. Disclose missing friction, calibration or collision guarantees.
7. Preserve failing examples and raw reports. A single screenshot cannot validate a solver.

## Accepted-state diagnostic sampling

The native Simulation inspector's **Sample final state** action is an explicit, one-shot, opt-in measurement. It runs immediately after one accepted step and does not change the predictor, collision coverage, contact refresh, adaptive rejection or accepted trajectory. It clones the collision world before detection, so the next accepted step cannot inherit a diagnostic broad-phase cache.

The sample reports:

- accepted physical time, step `dt`, total sample time, final-detection time and free-DOF force-audit time;
- active and retained final OGC contacts and, when a retained term exists, the minimum signed feature separation (mm); no contact term is reported as unavailable, not as a fabricated distance;
- RMS over free translational degrees of freedom of the backward-Euler residual (N);
- backend and freshness relative to the displayed state.

Each request carries a monotonic request id and the solver generation. The
result or failure carries the same identity, and the inspector only accepts the
latest request for the current generation. A reload or reset invalidates old
samples even when the physical timestamp matches, and a late result from an
earlier request cannot satisfy a newer one.

For a free vertex `i`, with previous state `(x-, v-)`, accepted state `(x+, v+)`, accepted `dt`, and physical force `F` at `x+` with final contacts:

```text
r_i = m_i (v_i+ - v_i-) / dt + damping * m_i * v_i+ - F_i
RMS = sqrt(sum_i |r_i|^2 / (3 * N_free))
```

`F` includes gravity, membrane, bending, sewing, contact and grab terms. Pinned vertices are excluded. `residual_rms_n = None` means there are no free vertices; `minimum_distance_m = None` means no retained contact term. An unavailable measurement is never displayed as zero.

This is not a KKT or constrained-equilibrium certificate: contact activation, directional movement bounds, the final detection and the accepted local solve are not re-solved. It is also not a per-color mixed gradient. The sample's contacts are a **fresh detection at the accepted positions**; the in-step `detection_ms` reports the earlier pre-update query used by the step, so the two contact sets must not be presented as identical.

At the compact barrier's activation radius the potential is only piecewise smooth: a very small penetration can carry a large force that a finite-difference step outside the support does not see. The sample reports the accepted-side force without regularizing or hiding it. Use final contacts strictly inside the support for gradient comparisons, and do not read a boundary sample as a convergence measurement.

Sampling is opt-in because the force audit costs `O(N_free)` local evaluations in addition to a fresh detection. Indicative medians of five steps on this machine, using the CPU test profile with `RAYON_NUM_THREADS=2`, were:

| scene | vertices | accepted step (ms) | sample (ms) | detection (ms) | free-DOF audit (ms) |
|---|---:|---:|---:|---:|---:|
| contact patch | 16 | 3.93 | 0.30 | 0.09 | 0.21 |
| seamed panels | 60 | 7.99 | 0.66 | 0.11 | 0.54 |
| T-shirt, 2 subdivisions | 2,454 | 155.68 | 26.61 | 2.42 | 23.70 |

The sample is outside `step_ms`; it is not part of the solver's complete accepted-step time, and the default path pays only one atomic request poll. These small local figures are not a portable performance gate. Keep the sample opt-in unless a reviewed gate on matched scenes supports a different default. A GPU-accepted state is audited by the CPU force path and labelled as such; no GPU residual or GPU timing is inferred. The sample is not a substitute for the offline energy audit or the solver comparison contract.

## Reproduce before competing

```powershell
.\scripts\check-cpu.ps1
# Explicit GPU opt-in; only when the workstation GPU is available:
.\scripts\turbo-perf.ps1 -Steps 360 -MeshMm 25 -UiPerf
```

The [current baseline](turbo-perf.md) includes RTX 3090 measurements, settings and limitations. Its before/after measurements predate the first Git commit; the historical executable and local raw output directories are not included in this source snapshot. Reproduce the current path for a fresh comparable baseline rather than claiming those historical runs are independently reproducible from Git.

Run `cloth-fit --gpu-sim --check-gpu-gradients` separately from timing. Extend the existing f64, shader and contact tests when a numerical change warrants it; ordinary CI must remain CPU-only.

## A contribution worth merging

Explain the mathematical/algorithmic change, the bounded problem it solves, and its failure modes. Attach the exact commands and a small table of **quality and complete cost** on the same inputs. Keep reproducible fixtures small and licensed. “Looks good and runs fast” is a starting observation, not the final evidence.
