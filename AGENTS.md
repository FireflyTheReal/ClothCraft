# Working instructions — MARVELOUS CLOTH

## Entry point

Read `AI_START_HERE.md`, `ROADMAP.md` and `README.md`, then `docs/handoff.md`, `docs/architecture.md` and the documentation of the module you are changing. `docs/research.md` and `docs/research-patterns.md` distinguish consulted articles, independent choices and methods that are not implemented. Do not present the cited work as available backends.

The current fitting backend is VBD + OGC: read `docs/ogc.md`. The CLI is CPU by default, `--gpu-sim` enables hybrid computation and `--reference` selects the historical Newton solver. The atelier uses `atelier.rs`. Do not confuse the compute timestamp, the full step time and the render time. OGC bounds apply from a separate initial state, with local seam exclusions that are explicitly documented.

The Turbo pass is documented in `docs/turbo-perf.md`. Preserve the conservative coverage of the pair cache, the recomputation of distances/bounds at every detection, the cooperative workgroup barriers and the worker clock independent of rendering. `scripts/turbo-perf.ps1 -UiPerf` reproduces the computation and the real window cadence; it explicitly uses the GPU.

## Execution

The documented local trials were authorized by the owner. For a new session, coordinate GPU use with the current request; a past authorization does not authorize all future workloads. Keep the default test suite on CPU and GPU tests opt-in. Limit small validations to two CPU jobs/threads.

Usual commands:

```powershell
.\scripts\check-cpu.ps1
cargo build --locked -j 2
```

The second command only compiles. The authorized GPU pass is reproduced with `scripts/first-fit.ps1 -Gpu -UiSmoke`; the CPU commands remain independent. Check the result of every command. No GPU test may be added to the suite that runs by default.

## Invariants

- 2D coordinates in metres are the material rest configuration. Never recompute the rest state from the draped 3D shape.
- A fabric material is shared; the grain belongs to each panel. Any change to the rest state, density or grain requires a consistent recompilation of the caches.
- Edit files reference panel/edge identifiers and material coordinates; no persistent vertex/triangle index.
- Preserve the `ReferenceSolver::step` API, `Project` v1 and the v1 rectangular templates. Polygons use `GarmentTemplate` v2, fitting uses `FittingProject` v1. See `docs/first-fit.md` and `docs/meshing.md` for caches and format changes.
- Delaunay is computed in the mesh metric, independent of the material. Preserve the contour constraints, check the quality bounds and invalidate any state cache after remeshing.
- Seams take part in the same implicit objective, the gradient, the Hessian and the metrics. Do not add arbitrary projection of positions after Newton.
- Parameters are dimensioned. Document the residual, convergence, penalty and limitations; a penalty is not a guarantee that no intersection occurs.
- Fiber guides are geometric attachments to the surface. They add neither physical yarns, nor contacts between yarns, nor measured calibration.
- Keep `model`, `energy`, `garment`, `contact` and `solver` independent of egui/wgpu. Editing and rendering consume the compiled model.

## Contributions

Preserve the user's files and changes. Do not publish, push or choose a distribution license without an explicit request from the owner. Test physical changes through verifiable properties: finite differences, objectivity, balances, convergence and limit behavior. Update the handoff guide and the limitations when a capability changes. Avoid empty modules, fake solver menus and abstractions that anticipate an implementation which does not exist.

## Editing and product research

`src/authoring.rs` contains the transactional CPU commands and the history. `src/atelier/authoring_ui.rs` and `gizmo.rs` are the graphical adapters. Read `docs/authoring.md` before changing them. A placement preview is not a new rest state; confirming it resets the caches. The public manuals actually read are listed in `docs/manual-study.md`. Do not announce the formats of `ROADMAP.md` as available. The comparison contract for new solvers is in `docs/solver-challenge.md`.

The unified board is in `atelier/board.rs`, 3D selection/sewing in `atelier/edges_3d.rs`, the Béziers in `garment/curves.rs`. Preserve the sharing of edge identifiers between views. `layout_2d` is excluded from physics; the handles stay in the local rest state. Do not disable `serde_json/float_roundtrip` without checking mesh stability on reload.
