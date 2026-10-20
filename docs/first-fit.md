# First fitting: mannequin, T-shirt and dress

**Archive of the Newton pass before OGC.** The script now explicitly uses `--reference`. Newer patterns are wider; do not compare them with these measurements as if they were the same scene. Current GPU computation and workspace interactions are documented in [OGC](ogc.md) and [OGC validation](ogc-validation.md).

`cloth-fit` loads a static mannequin and two independent test patterns: a T-shirt with integrated sleeves (eight connections) and a flared sleeveless dress (four connections). Neck, cuffs/armholes and hem remain open. There are no seam allowances or industrial grading rules.

## Reproduce

```powershell
# CPU meshing, solver and exports only
.\scripts\first-fit.ps1 -Steps 5 -MeshMm 25

# GPU opt-in: measured rendering and native-window capture
.\scripts\first-fit.ps1 -Steps 5 -MeshMm 25 -Gpu -UiSmoke

# Explore the result; the window starts paused
cargo run --locked -j 2 --bin cloth-fit -- --project output/first-fit/tshirt/fitting-project.json --steps 0 --view --output output/viewer
```

The T-shirt / Dress actions load scenes from `output/first-fit/` when present, otherwise presets. Target size, anisotropy, axis and remesh/reset control [constrained Delaunay](meshing.md). Remeshing restarts from the initial arrangement; it does not transfer the old drape to the new topology. Per-panel geometric reports are in `run.json`. Labels and panel locations have evolved since this historical pass.

`--mesh-mm 15` gives a denser test; `--steps 0` prepares/exports without advancing physics. `--gpu-benchmark` renders 12 warm-up and 60 measured frames at 1200×1200. `--ui-smoke` opens, captures and closes the window. None of these GPU paths runs in ordinary tests.

## Model and result scope

Physics in this historical pass is **CPU f64** Newton/PCG with integrated sewing. Its default is **incremental assembly relaxation**: each step starts with zero velocity and recorded time is pseudo-time. A few converged steps establish neither garment equilibrium nor validated dynamics. `quasistatic: false` enables dynamic implicit Euler in a project.

Prepositioning brings boundaries closer in depth and moves vertices away from the body silhouette before solving. It does not alter the rest pattern. It can introduce stretch and face intersections; sewing gaps may therefore initially increase as cloth relaxes.

The body uses a static CPU BVH and local signed distance to the nearest triangle. Historical contact is a **vertex penalty**: no CCD, friction, face/face contacts or self-collision. Its local sign is not a global inside/outside oracle for arbitrary OBJ files. Denser triangulation improves sampling but does not guarantee nonintersection. `contact_clearance` can add a numerical envelope (default zero); penetration metrics then measure envelope violation.

A rejected step restores the entire prior state. At most seven attempts halve dt; subsequent steps retain that reduction. `run.json` records requested/accepted steps, status, possible error, wall time including rejected attempts and mean accepted-solve time. `metrics.jsonl` stores residuals, tolerance, dt, seam gaps and penetrations. Failure still exports the last accepted state and returns a nonzero code.

The GPU benchmark measures **static rendering**, not physics: GPU timestamps for the 3D pass and, separately, CPU preparation + submission + waiting. It excludes window presentation and does not measure interactive FPS. The script uses a CPU release build for solving and an optimized desktop development build for rendering. `gpu-render.json`, `preview.png`, `front.png`, `back.png` and `ui.png` are in `gpu/`; CPU measurements remain at each case root.

## Results of 14 October 2026

Local execution, two CPU threads, RTX 3090 / Vulkan, NVIDIA driver 616.56. Full values: [first-fit-results.json](first-fit-results.json).

| Case, 25 mm target | Triangles | Vertices | Minimum angle | Accepted steps | Maximum seam gap after 5 steps | Maximum vertex penetration |
|---|---:|---:|---:|---:|---:|---:|
| T-shirt | 3,140 | 1,704 | 25.08° | 5/5 | 30.43 mm | 5.05 mm |
| Dress | 5,664 | 3,000 | 25.31° | 5/5 | 47.74 mm | 5.97 mm |

Accepted step objectives converge, but garments are **neither settled nor intersection-free**. Five-step captures still show body intersections. The previous meshes had only 384 triangles for the T-shirt and 256 for the dress.

Mean accepted-solve times are 3.19 and 4.05 s respectively; wall times for all five steps including rejected attempts are 36.22 and 87.70 s. These small scenes do not run in real time with this CPU reference.

The GPU rendering pass has median 0.0184 ms and P95 0.0195 ms in both cases; CPU preparation/submission/waiting have medians 6.95 and 7.72 ms. These are not solver timings and do not predict GPU physics performance.

A **no-simulation** 15 mm pass gives 8,768 triangles and 4,612 vertices for the T-shirt, minimum angle 25.16°. A 2:1 anisotropic pass at 90° and 25 mm gives 1,596 triangles, metric minimum angle 25.54° (Euclidean 13.44°). Captures verify mesh density/orientation and show the initial arrangement.

## Formats and modules

- `GarmentTemplate` v2 adds `polygon` (points, segment names, meshing settings), material `pin_points` and `BoundaryRef.edge`. `pattern.width/height` remains the presentation box. Semantic seam IDs/ranges survive refinement.
- `FittingProject` v1 holds template, avatar path, contact, solver, mode and optional state cache. Relative avatar paths resolve against the JSON. This historical pass saved absolute paths; current portable documents use the [document contract](documents-and-sewing.md).
- `avatar.rs`: OBJ surface/BVH; `fitting.rs`: arrangement and transactional steps; `garment/meshing.rs`: Delaunay; historical `fitting_editor.rs`: interface; `gpu_bench.rs` and `render.rs`: display/measurements. The current workspace uses `atelier.rs`.

## Mannequin provenance

Official MakeHuman base mesh, revision `a8bc2d54ff0ac92e78ff71431b1023eda42bf482`, downloaded 14 October 2026. The [official license separates program and assets](https://raw.githubusercontent.com/makehumancommunity/makehuman/a8bc2d54ff0ac92e78ff71431b1023eda42bf482/LICENSE.md) and places the base mesh under CC0. No MakeHuman program code was incorporated.

`assets/makehuman/` contains the source OBJ, derivative, license and provenance. `python scripts/prepare-avatar.py` extracts only the `body` group, removes helpers, triangulates quads and normalizes height to 1.75 m with feet at floor level. Result: **13,380 vertices, 26,756 triangles**. `measurements.json` retains the transform, landmarks and source SHA-256.

At the time of this initial pass, the repository was unpublished and its source license had not been selected. The owner subsequently selected MIT; see the current [LICENSE](../LICENSE).
