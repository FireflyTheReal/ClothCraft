# Workspace polish and interaction cost

14 October 2026. This pass targets the authoring UI, visibility queries and 2D rendering. It does not change physical energies, contact bounds, timestep, iterations or mesh density. No solver FPS gain is claimed.

## What changed

- A restrained dark workspace with grouped drawing/sewing/arrangement tools, a linked piece browser and a contextual inspector. The 2D pane starts at 45% of the available workspace instead of a fixed 610 px. The toolbar wraps on smaller windows.
- Single-step/reset live under **Simulation**; the reset explicitly says it restores the flat arrangement. **Vue générale** reframes 3D. **Fichiers & formats** describes actual outputs and their destination without advertising future adapters as available tools.
- Committed seams have stable colors in 2D, 3D and the list. Three sparse links show each background relationship; selection/hover shows nine correspondences. Small endpoint ticks and a labelled pink preview show which ends are joined before commitment. **Affichage → Liens de couture 2D + 3D** hides background links, retaining the active pair.
- `CompiledSeam::guides` is a transient material-attachment cache compiled with the same trim, reversal and notch map as the seam. It is rebuilt on edits/remeshing, never serialized as vertex IDs, and adds no physical constraints. The overlay follows drape and board-layout previews; it is not depth-tested and can be visible through geometry.
- Avatar visibility now traverses the existing static BVH, with no per-query allocations, instead of testing every avatar triangle. Cloth-grab, arrangement picking and 3D boundary picking share it. Leaf tests retain the original two-sided triangle predicate and exclusive distance threshold. This is independent of OGC detection; other-cloth occlusion still scans cloth triangles.
- 3D boundary projection reuses one camera matrix per pass. The 2D board transforms/uploads each indexed vertex once, instead of once per triangle corner. A 2,454-vertex / 4,570-triangle shirt therefore uses 2,454 rather than 13,710 board vertices (82.1% fewer), with identical mesh resolution. Triangle indices remain unchanged.
- Dashed links have a bounded subdivision count (about 32 dashes per correspondence) even when pieces are far apart. Their count does not grow with mesh refinement.

Public CLO/Marvelous screenshots and official manuals informed the workspace hierarchy and sewing feedback; see [sources actually inspected](manual-study.md). No proprietary UI assets were imported. The [interchange plan](interoperability.md) separates supported input/output from future formats and gives each adapter acceptance criteria.

## Validation and measured scope

Local machine: Windows, AMD Ryzen 7 2700X, NVIDIA RTX 3090. CPU checks use two jobs/test threads. The complete CPU script passes: **80 tests**, no-default-features Clippy and all five template validations. Desktop all-target Clippy and the release build also pass. The final native authoring scenario completes **41/41 actions**, including simulated new-panel motion and export/reload. Captures of the saved shirt were visually checked at **1600×980** and **1100×700**; both views, toolbar and inspector remain accessible. The README image is our own native capture of that saved state.

The CPU visibility probe reports **1,851.47 ms exhaustive → 0.471 ms BVH** per 4,096-ray batch, with identical results. There are 251 blocked rays and 3,845 misses; most rays are rejected by high-level bounds. This unusually large microbenchmark ratio is specific to sparse avatar visibility and must not be advertised as a whole-editor or physical-solver speedup. It excludes collider construction and leaves cloth-triangle picking unchanged.

The native realtime check uses the same saved 2,454-vertex shirt, dt = 1/60 s and 16 iterations, with one second of warmup and five seconds measured. The first polished run gave 59.99 UI frames/s but only 42.65 physical steps/s (0.711×), failing the physical-time threshold. A fresh release build of baseline commit `66052f3` was then compared sequentially against the polished executable on that identical snapshot:

| Build | UI frames/s | UI interval P95 | Physical steps/s | Simulated/wall time | Realtime check |
|---|---:|---:|---:|---:|---|
| Baseline `66052f3` | 57.80 | 25.05 ms | 45.24 | 0.754× | **Failed** |
| Polished, seam links enabled | 59.24 | 20.94 ms | 51.46 | 0.858× | **Failed** |

These are short observations on a shared station, **not a controlled solver speedup**. After the first run, `nvidia-smi` still reported 34% GPU utilization and 13,277 MiB occupied with ClothCraft closed. Other applications remained running; their contribution was not isolated or modified. Both versions miss the >=0.95× physical-time criterion. UI cadence passes the >=55 frames/s criterion. Preserve this failure rather than equating a responsive window with realtime simulation.

[Raw validation report](workspace-validation.json) retains all three runs, CPU probe results, action checks and the starting file SHA-256. The saved drape is a local fixture under ignored `output/`, not a distributed benchmark asset; compare versions on an identical saved project and disclose its state/settings. The public CPU visibility probe is reproducible directly from the bundled avatar.

## Reproduce

CPU-only checks and visibility microbenchmark:

```powershell
.\scripts\check-cpu.ps1 -Jobs 2
cargo clippy --locked --all-targets -j 2 -- -D warnings
cargo run --release --locked --no-default-features -j 2 --example picking_probe
```

The probe casts 4,096 deterministic rays across the 26,756-triangle bundled avatar. It first checks every result against exhaustive traversal, then averages three batches per implementation. Many rays miss the body; the JSON includes the hit count. This is a CPU visibility microbenchmark, **not a simulation, window-FPS or end-to-end picking benchmark**. BVH construction is excluded because the editor already owns the collider.

Native checks use the GPU explicitly; coordinate access before running them:

```powershell
cargo build --release --locked -j 2 --bin cloth-fit
target/release/cloth-fit.exe --steps 0 --ui-authoring-smoke --output output/workspace-polish/authoring
target/release/cloth-fit.exe --steps 0 --ui-smoke --output output/workspace-polish/default
```

`--ui-authoring-smoke` exercises 41 actual egui pointer actions: rectangle/Bézier drawing, anchor/handle edits, 2D layout/rotation, 3D translation and all rotation rings, 2D/3D sewing selection, undo/redo, commitment on Play, seam deletion preserving the drape, fabric assignment, export/reload and motion of a new piece. The scenario reports `passed` in `authoring.json`; it is an interaction regression, not a garment-quality benchmark.

Set `CLOTHCRAFT_SMOKE_SIZE=1100x700` for the compact native capture, then restore the previous environment value. The override only applies to explicit smoke/performance modes; normal startup remains 1600×980. Captures are `ui.png` in the chosen output directory. A saved fitting project can be supplied with `--project path/to/fitting-project.json`; `--steps 0` displays its saved state, not a newly simulated result.

## Limits and next work

This is still an early prototype. There is no portable save/open dialog, autosave, multi-selection, calibrated printing, industrial format adapter or complete pattern-construction toolkit. Link lines themselves are not clickable: select a boundary or a seam in the list. The eight-color palette can repeat, so labels and selection remain authoritative. A reset or 3D arrangement is not collision repair.

The physical bottleneck remains hybrid CPU contact detection. Continue physical optimization with [quality/cost comparisons](solver-challenge.md), not by reducing iterations or resolution without reporting the resulting quality. Sparse scene timings cannot establish performance on dense or layered garments.
