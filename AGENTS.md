# Working instructions — ClothCraft

## Start here

Read `AI_START_HERE.md`, `ROADMAP.md` and `README.md`, then `docs/handoff.md`, `docs/architecture.md` and the documentation for the module being changed. `docs/research.md` and `docs/research-patterns.md` distinguish consulted papers, independent design choices and unimplemented methods. Do not describe cited work as available backends.

The current fitting backend is VBD + OGC: read `docs/ogc.md`. The CLI defaults to CPU; `--gpu-sim` enables hybrid computation and `--reference` selects the historical Newton solver. The workspace uses `atelier.rs`. Distinguish compute timestamps, complete step times and rendering times. OGC bounds apply from an initially separated state, with explicitly documented local sewing exclusions.

The Turbo work is documented in `docs/turbo-perf.md`. Preserve conservative pair-cache coverage, distance/bound recomputation at every detection, cooperative workgroup barriers and the worker clock independent of rendering. `scripts/turbo-perf.ps1 -UiPerf` measures computation and actual window cadence; it explicitly uses the GPU.

## Execution

The owner authorized the documented local experiments. For a new session, coordinate GPU use with the current request; past permission does not authorize all future workloads. Keep the default test suite on CPU and GPU tests opt-in. Limit small local checks to two CPU jobs/test threads.

Typical commands:

```powershell
.\scripts\check-cpu.ps1
cargo build --locked -j 2
```

The second command only compiles. The authorized GPU pass is reproduced with `scripts/first-fit.ps1 -Gpu -UiSmoke`; CPU commands are independent. Check every command result. Never add GPU tests to the default suite.

## Invariants

- The 2D coordinates in metres define material rest geometry. Never reconstruct rest geometry from the draped 3D shape.
- Fabric properties are shared; grain belongs to each panel. Changing rest geometry, density or grain requires coherent recompilation of dependent caches.
- Editable files reference panel/boundary IDs and material coordinates, never persistent vertex/triangle indices.
- Preserve `ReferenceSolver::step`, `Project` v1 and rectangular templates v1. Polygons use `GarmentTemplate` v2; fitting uses `FittingProject` v1. See `docs/first-fit.md` and `docs/meshing.md` for caches and format changes.
- Delaunay triangulation uses the meshing metric, independently of material properties. Preserve boundary constraints, check quality bounds and invalidate state caches after remeshing.
- Sewing belongs to the same implicit objective, gradient, Hessian and metrics. Do not add arbitrary position projections after Newton.
- Parameters have physical units. Document residuals, convergence, penalties and limitations; a penalty is not a nonintersection guarantee.
- Fiber guides are geometric surface attachments. They add no physical yarns, yarn contacts or measured calibration.
- Keep `model`, `energy`, `garment`, `contact` and `solver` independent of egui/wgpu. Editing and rendering consume the compiled model.

## Contributions

Preserve user files and existing changes. Do not publish, push or choose a distribution license without an explicit request from the owner. Test physics changes through falsifiable properties: finite differences, objectivity, balances, convergence and limiting behavior. Update the handoff and limitations when a capability changes. Avoid empty modules, fake solver menus and abstractions for implementations that do not exist.

Write code comments, UI text and documentation in English; localization is future work. Preserve serialized identifiers, existing document compatibility, source quotations and attribution. Report observed results rather than claiming a test passed because it was intended to pass.

## Editing and product research

`src/authoring.rs` owns CPU transactions and history. `src/atelier/authoring_ui.rs` and `gizmo.rs` are graphical adapters. Read `docs/authoring.md` before changing them. A placement preview is not new rest geometry; committing it resets dependent caches. Public manuals actually consulted are recorded in `docs/manual-study.md`. Do not advertise planned formats from `ROADMAP.md` as supported. The solver comparison contract is in `docs/solver-challenge.md`.

The unified board is in `atelier/board.rs`, 3D selection/sewing in `atelier/edges_3d.rs` and Bézier operations in `garment/curves.rs`. Preserve shared boundary identities between views. `layout_2d` is excluded from physics; handles remain in local rest coordinates. Do not disable `serde_json/float_roundtrip` without checking mesh stability after reload.
