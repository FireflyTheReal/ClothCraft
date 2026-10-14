# Fabric controls and dual-BVH validation — 2026-10-14

Historical eight-iteration results, before the subsequent [adaptive initialization and supported-cloth correction](adaptive-initialization.md). Current default: 16 iterations. Equal trajectories in this optimization comparison did not establish accurate supported-cloth dynamics.

This pass adds [fabric controls with explicit evidence](fabrics.md) and a conservative dual-hierarchy candidate traversal. [Research and implementation boundaries](performance-research.md). It preserves the corrected GPU inertial prediction from [the previous pass](apron-validation.md).

## Measured performance

RTX 3090, Vulkan, Windows, two CPU threads, release build; eight VBD iterations, dt = 1/60 s. Baseline: commit `8cfe4ec`. No compilation ran during measurements. Meshes, material values, contact settings and solver iterations stayed identical. Compact raw measurements and source hashes: [perf-fabrics-validation.json](perf-fabrics-validation.json).

Three alternating before/after runs each advanced the same saved private apron scene by 90 steps: 3,139 vertices, 5,823 triangles, three panels and nine seams.

| Full wall time per step | Before | After |
|---|---:|---:|
| Run 1 | 31.744 ms | 21.674 ms |
| Run 2 | 30.937 ms | 23.342 ms |
| Run 3 | 31.598 ms | 24.108 ms |
| Mean of runs | **31.426 ms** | **23.041 ms** |

The observed reduction is **26.68%**, or **1.364× throughput**, including host work and metrics logging. All three paired final states have **exactly equal serialized positions and velocities** (maximum component difference zero). Final maximum seam gap is 30.523 mm in both versions. Equality on this short scene is a regression result, not a general proof of identical trajectories or seam convergence.

Actual native-window checks use one second warmup and five measured seconds:

| Scene | UI frames/s | Physical steps/s | Simulated/wall time | Frame interval P95 | Realtime check |
|---|---:|---:|---:|---:|---|
| T-shirt, baseline | 59.951 | 50.159 | 0.836× | 19.762 ms | Fail |
| T-shirt, final | 60.007 | 60.206 | 1.003× | 20.338 ms | **Pass** |
| T-shirt + apron, final | 60.004 | 46.249 | 0.771× | 19.705 ms | **Fail** |

The T-shirt starts from the same saved 6 s / 25 mm snapshot for both versions. The short measurement can include a boundary step, hence slightly over 60 physical steps/s; the worker's target remains 60 Hz. Acceptance requires ≥55 UI frames/s and ≥0.95× delivered physics. The apron remains below realtime despite a responsive window. These are bounded local measurements, not universal FPS guarantees.

The first optimized apron run averages 0.367 ms refit, 10.154 ms candidate construction and 5.864 ms exact narrow phase/incident construction. Coloring and other host work add cost. `gpu_compute_ms` is only hardware compute, not full step time. CPU contact detection remains the main target. Smaller BVH leaves and a larger cache skin were tried but not retained: final leaves contain at most eight primitives and skin remains the contact radius (3 mm here).

## Correctness and authoring

- **74 CPU tests pass**, including exhaustive dual-tree pair coverage/uniqueness after deformation and finite-difference verification that each preset's stated extension yields the requested 100 N/m traction in the actual membrane energy.
- Surface mass equals rest area × density; single-panel changes detach shared fabric, retain other panels, preserve compatible positions/velocities and survive JSON round-trip. Invalid edits are transactional.
- Rustfmt, CPU and desktop Clippy with warnings denied, and five template fixtures pass. Ordinary checks do not initialize a GPU.
- The opt-in GPU free-fall / topology-growth / trust-bound test passes.
- **35 native pointer actions pass**, including drawing, 2D/3D sewing, gizmos, applying denim without losing the drape, export/reload and actual new-panel simulation. The captured inspector was visually inspected.
- Two explicit GPU/CPU gradient checks on the exported mixed-fabric editor scene pass (946 vertices; maximum reported absolute errors 0.000758 N and 0.001247 N). These diagnostic runs are excluded from timing.

This does not calibrate the estimated elastic parameters, add friction, certify stiff-material convergence or guarantee garment fit. Mass/thickness provenance and all modeling limits remain visible in [fabrics.md](fabrics.md).

## Reproduce

```powershell
.\scripts\check-cpu.ps1
cargo clippy --locked -j 2 --all-targets -- -D warnings
cargo build --release --locked -j 2 --bin cloth-fit

# GPU opt-in, only when the machine owner has made the GPU available:
cargo test --locked -j 2 --test gpu_motion -- --ignored --test-threads=1
.\target\release\cloth-fit.exe --steps 0 --ui-authoring-smoke --output output/fabric-authoring
.\target\release\cloth-fit.exe --project output/fabric-authoring/fitting-project.json --gpu-sim --check-gpu-gradients --steps 2 --output output/fabric-gradients

# Locally retained snapshots, when available:
.\target\release\cloth-fit.exe --project output/apron-ready/fitting-project.json --gpu-sim --steps 90 --threads 2 --output output/fabric-apron-perf
.\target\release\cloth-fit.exe --project output/turbo-repro/tshirt/fitting-project.json --steps 0 --ui-perf-smoke --output output/fabric-tshirt-ui
.\target\release\cloth-fit.exe --project output/apron-ready/fitting-project.json --steps 0 --ui-perf-smoke --output output/fabric-apron-ui
```

The private apron source is preserved locally and is not a public benchmark fixture. On another machine, `scripts/turbo-perf.ps1 -Steps 90 -UiPerf` generates new public-preset trajectories and their own reports; those measurements are not the saved-scene comparison above. Do not ignore the nonzero exit of a failed performance check.
