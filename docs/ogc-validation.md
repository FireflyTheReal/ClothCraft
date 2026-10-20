# OGC / VBD validation — 14 October 2026

**Archive of the initial implementation.** [Turbo](turbo-perf.md) records subsequent optimization; newer work is indexed in [the handoff](handoff.md). Values below remain pre-optimization measurements.

Actual local runs on **NVIDIA GeForce RTX 3090, Vulkan, driver 616.56**, Rust release build, two CPU threads. Compact raw results: [ogc-validation.json](ogc-validation.json); local snapshots, logs and captures: `output/ogc-validated/`.

## Garment tests

Two flat panels initially separated by 530 mm, static 26,756-triangle mannequin, Delaunay target 30 mm. Uncalibrated reference membrane/bending. Normal OGC: radius 3 mm, stiffness 100,000 N/m per pair, relaxation 0.45. Sewing: 50,000 N/m². 360 steps at 1/60 s, eight VBD iterations: **6 simulated seconds**.

| Measurement | T-shirt | Dress |
|---|---:|---:|
| Cloth vertices / triangles | 1,744 / 3,212 | 2,808 / 5,324 |
| Wall time, 360 steps | 17.79 s | 20.87 s |
| Complete mean step | 49.43 ms | 57.98 ms |
| Median, final 60 steps | 53.10 ms | 55.98 ms |
| P95, final 60 steps | 67.43 ms | 79.82 ms |
| CPU detection + coloring, final median | 34.96 ms | 37.13 ms |
| GPU compute only, final median | 14.83 ms | 15.05 ms |
| Final maximum seam gap | 2.78 mm | 2.53 mm |
| Minimum distance at last detection | 1.66 mm | 2.71 mm |

**This historical hybrid backend did not sustain 60 physical Hz.** Final throughput was about 18–19 steps/s at dt 1/60 s. An asynchronous UI does not make that real-time simulation. Timings depend on scene, contacts and machine activity. Medians/P95 describe only the ends of these trajectories, not all possible scenes.

The largest T-shirt seam gaps occur at underarm junctions; shoulders are about 0.37–0.38 mm. The dress remains below 2.54 mm. These are finite penalties, not exactly closed seams. Final snapshots were reopened and passed initial-intersection diagnostics for non-excluded pairs. This floating-point diagnostic is not exact geometric certification.

## Checks

- **54 CPU tests**, including historical suites. OGC: law continuity, gradient/Hessian finite differences, action/reaction, objectivity, contact layers, deduplication, reciprocal VF and EE, accumulated bounds, intersecting initial states, static/dynamic primitive order and a nearly coplanar shared-vertex regression.
- VBD: analytical free fall, translation-preserved rest, grab forces and transactional failure. Naga parses/validates both shaders without a GPU.
- All local GPU forces compared with f64 AD on contacting snapshots: maximum absolute errors **1.084e-4 N** for the T-shirt and **1.434e-3 N** for the dress. Energies also checked. Geometry is identically rounded; force tolerance is 3 mN + 0.3% of norm. GPU Hessians are positive approximations, not exact CPU Hessians.
- Grab test in a **real egui window**: pointer events, picked triangle, accepted GPU attachment step, maximum displacement 1.35 mm. `interaction/interaction.json`: `passed=true`. This checks event → picking → worker → solver → display, not comprehensive usability.
- CPU/desktop Clippy without warnings and formatting checks pass. No GitHub CI execution is claimed.

Reopening exposed a false positive for two triangles sharing a vertex: a tiny determinant produced an intersection time near 1. The diagnostic now skips segments with an **endpoint topologically shared** with the tested triangle; opposite edges remain checked, and coplanar overlap also uses VF/EE distances. A test retains the problematic coordinates.

## Rendering and captures

Real T-shirt/dress workspaces were opened and captured. Library, 3D view, 2D patterns and properties are separated; garment/avatar use interpolated normals. Display smoothing does not change geometric density.

At 1200×1200, 12 warm-up frames then 60 measurements: median rendering preparation/submission/wait **1.28 ms** T-shirt and **1.09 ms** dress; GPU rendering pass alone **0.018–0.020 ms**. These are static offscreen renders, neither solver measurements nor evidence of window FPS. Avatar caches and GPU camera transforms avoid rebuilding it each frame.

## Reproduce

```powershell
.\scripts\check-cpu.ps1
.\scripts\ogc-benchmark.ps1 -Gpu -UiSmoke -Steps 360 -MeshMm 30

.\target\release\cloth-fit.exe --project output/ogc-benchmark/tshirt/fitting-project.json --gpu-sim --check-gpu-gradients --steps 1 --output output/force-check

.\target\release\cloth-fit.exe --project output/ogc-benchmark/tshirt/fitting-project.json --steps 0 --ui-interaction-smoke --output output/interaction-check
```

Gradient checks include extra CPU AD work and must not be used as benchmarks. The grab command opens and automatically closes a window. Captures alone demonstrate neither convergence nor global nonintersection.

## Technical follow-up at this stage

Priorities were GPU VF/EE detection and BVH refit, resident solver buffers and better GPU occupancy for small color groups. Retain f64 as an oracle. Subsequent work: refreshing bounds within a GPU step, friction, stronger sewing constraints and explicit thickness treatment. Read [current contracts and differences from the paper](ogc.md) for implemented scope.
