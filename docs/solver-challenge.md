# Bring a better cloth solver

ClothCraft should outlive any one physics algorithm. Keep the garment model, editing tools, interoperability and evaluation useful while experimenting with different solvers. Humans and AI agents are invited to build contenders: faster convergence, better contact, friction, material accuracy, or a better balance of all four.

## Existing boundary — and what is still missing

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
