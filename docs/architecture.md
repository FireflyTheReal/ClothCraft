# Architecture and invariants

The repository remains one Rust package and three executables. The CPU numerical modules import neither eframe nor wgpu. The `desktop` feature adds the renderer, the `gpu_vbd` compute backend and the interfaces; it can be disabled to compile only the CPU core.

## Data path

```text
GarmentTemplate JSON
  fabric properties + panels (rest geometry / grain / placement) + seam graph
       | compile(): validation, triangulation, material binding, seam quadrature
       v
CompiledGarment = Mesh + materials + vertex material IDs + seams + panel ranges
       | initial_state.clone()
       v
ReferenceSolver::step_garment() -> State + Metrics
       |                          |
       +-- OBJ / fiber frames ----+-- material-space attachments

Project JSON v1 -> Pattern::build() -> ReferenceSolver::step()
                                      |
                          same private Newton/PCG implementation
```

The historical editor uses `Project`, a single panel. `cloth-fit` uses `FittingProject`: multi-panel template, avatar, settings and a position cache with a mesh signature. The CLI and the interface consume the same `FittingScene`, without duplicating the solver.

The current `cloth-fit` path goes through `VbdSolver` + `OgcWorld`, or `GpuVbd::step` with the same OGC topology and the same energies. The Newton diagram above remains that of the `--reference` backend. The atelier worker receives a generation number: a result computed on an old mesh never replaces the current scene. It owns the physical state and its fixed-step clock, independently of the egui refresh. A single output snapshot is kept; control updates never inject the old displayed state back. GPU resources and material terms are reused, and the OGC candidate cache never replaces the recomputation of distances and bounds. See [Turbo](turbo-perf.md).

## Responsibilities

| Module | Role | Inputs / outputs |
|---|---|---|
| `authoring.rs` | Transactional commands, history and rigid placements | template + command → validated template + compiled model |
| `fabrics.rs`, `atelier/fabrics_ui.rs` | Documented presets, stretch conversion under load and controls | SI properties → compilation through `Command::Fabric`, without changing the rest state |
| `atelier/authoring_ui.rs`, `atelier/gizmo.rs` | Drawing, seam choice, selection and gizmos | interactions → commands; preview without remeshing per frame |
| `model.rs` | Rectangular rest geometry and FEM mesh | `Pattern` → `Mesh`, mass, hinges |
| `blocksolve/` | Assembled mixed block-CSR operator and PCG with nodal block-Jacobi preconditioning | element contributions + mass → assembled operator → Newton direction |
| `garment/template.rs` | Authoring model, validation, compilation | `GarmentTemplate` → `CompiledGarment` |
| `garment/polygon.rs`, `garment/meshing.rs` | Validated contours, constrained Delaunay and metric | points/edges → UVs and triangles, quality control |
| `garment/seam.rs` | Edge correspondence and joining energy | rest lengths → quadrature, elements |
| `garment/embedding.rs` | Geometric attachments in pattern coordinates | `MaterialPath` → temporary barycentrics |
| `garment/export.rs` | CPU artifacts without interface state | per-panel OBJ, SVG, fiber directions |
| `math.rs`, `energy.rs` | Second-order AD, membrane and bending | current geometry + rest state → energy/derivatives |
| `contact.rs` | Vertex/sphere penalty | discrete contact, per-vertex thickness |
| `avatar.rs` | OBJ, static BVH and signed local distance | avatar → implementation of the Collider trait |
| `fitting.rs` | Arrangement, cache and transactional steps | FittingProject → FittingScene |
| `solver.rs` | Implicit Euler, Newton/PCG, Armijo | compiled model + state → state/metrics |
| `project.rs` | Legacy single-panel scene format | `Project` JSON v1 |
| `bin/cloth-template.rs` | Windowless CLI | example / validate / bake / simulate |
| `editor.rs`, `render.rs` | Interface and display | remain consumers of the CPU model |
| `ogc/` | Admissible regions, VF/EE, barriers and bounds | topology + positions → contacts + conservative radii |
| `vbd.rs` | Implicit Euler by vertex blocks, coloring with contacts | same FEM/seam energies, f64 reference |
| `vbd/prediction.rs` | Adaptive CPU/GPU initialization, history of accepted steps | previous velocities/acceleration → initial estimate, inertial target unchanged |
| `garment/diagnostics.rs` | Extension measurement independent of rendering | material rest state + state → percentile by area and maximum |
| `gpu_vbd.rs`, `vbd.wgsl` | Local f32 solve, exact gradients, approximate positive Hessians | CPU candidates/bounds → GPU compute → validated state |
| `picking.rs` | Rays and barycentric attachment | mouse → grab force, without teleportation |
| `bin/cloth-fit.rs`, `atelier.rs`, `gpu_bench.rs` | CLI, atelier and render measurement | projects / reports / opt-in captures |

## Contracts to preserve

**Rest state.** `Mesh.uv` is local to the panel and expressed in metres, without an atlas or [0,1] normalization. `Triangle.rest`, `inv_dm` and `area` derive only from the pattern. Rigid placements affect neither this data nor the masses.

**Materials.** The template separates `FabricProperties` and `Panel.grain_degrees`. Compilation creates a `Material` oriented per panel. Triangles and hinges reference this material; `vertex_material` provides the thickness to contacts. Density is already integrated into mass: rebuild the model when it changes, do not modify only the table of compiled materials.

**Topology.** Panels keep distinct vertices at seams. Vertex/triangle ranges identify each panel. The compiler rejects seams that have overlapping intervals of the same edge; a shared corner remains possible. Supporting multi-layer seams will require a richer relation model.

**Newton reference.** Its two APIs enter the same private `Problem`. Materials, inertia, gravity, damping, contacts and seams are assembled together. Fixed constraints eliminate the corresponding DOFs. Errors keep the previous state; reaching the iteration budget produces a state with `converged=false`. Seam metrics are added, not substituted for the residuals.

**Assembled operator.** `System::assemble_operator` and `System::apply` must describe the same operator; `blocksolve_assembly_matches_the_matrix_free_operator` is the guard. The blocks are nodal and indexed `3 * vertex + component`: `Element::from_jet` must remain the single source of DOF numbering. The strict upper triangle is enough because the contributions come from symmetric Hessians; do not store both triangles, and do not additionally traverse the mirror pair, which would double every off-diagonal block. The nodal diagonal must remain invertible; otherwise `System::pcg` falls back to `pcg_point_jacobi`. Fixed dofs remain identity rows through `apply_dirichlet`. Nodal block-Jacobi preconditioning brings an iteration gain only when the nodal blocks have a real off-diagonal component; on the T-shirt fixtures it is neutral, and no time gain is claimed. See [the CLO3D analysis](clo3d-reverse-engineering.md).

**VBD/OGC.** The same energies are minimized by vertex blocks. Coloring includes all dependencies of the elements, contacts and grabs. Bound centers change only on a new detection; local proposals are truncated and then checked energetically. The f32 GPU backend verifies the results and the bounds before modifying `State`. A fixed budget does not mean convergence. The CLI's VBD settings are recorded in `run.json`; `FittingProject.solver` keeps the historical Newton settings.

Adaptive initialization keeps a history of the accepted state in `VbdSolver`, shared by the CPU/GPU paths. It does not belong to the edit files; a rebuild or a different external state invalidates it. The damping-corrected acceleration only chooses the solver's starting point; gravity remains whole in the objective. See [the false-elasticity diagnosis](adaptive-initialization.md). The default budget is 16 iterations since this correction.

**Attachments.** `MaterialPath` keeps a panel ID and 2D points. `embed_path` splits segments at triangle crossings before barycentric interpolation: a guide therefore follows a fold instead of crossing the space between its endpoints. `EmbeddedPoint` values are caches to rebuild after remeshing; they do not automatically detect a different mesh of the same size. The current search walks the triangles, which suits small patterns; a spatial index may replace it.

**Units.** Lengths m, mass kg, time s, force N, energy J, density kg/m², membrane N/m, bending N·m, seam coefficient N/m². Warp/weft guides are not an explicit yarn model.

## Localized evolutions

Since the [jitter correction](jitter.md), `ContactSet.contacts` also contains nearby admissible interactions whose energy is still zero: the activation distance remains `r`, the selection goes up to `rq`. Coloring and incidence include these dormant terms. `active_contacts` / `VbdMetrics.contacts` indicate those with positive energy at detection; `contact_terms` gives the cost in retained terms. Canonical identifiers are sorted/deduplicated before coloring. The GPU works in a translated frame, restored on readback; the CPU state and contacts remain global in f64. Do not forget this frame change for a new target or a GPU diagnostic.

1. Extend Bézier contours to holes, then add a spatial-resolution metric.
2. Transfer the state during dynamic remeshing; today explicit remeshing rebuilds the initial arrangement.
3. Extend current editing (points/Bézier handles, 2D/3D seams, gizmos, history) to holes, dimensions and partial seams.
4. Port OGC VF/EE detection to GPU, add friction and dedicated seam handling. A seam does not authorize excluding all collisions between sewn panels. OGC bounds preserve a separate state; they are not a repair of initial intersections.
5. Evolve the constitutive laws from measurements or homogenization. Keep the material coordinates and objectivity tests. Do not invent a calibration.

The goal for backend evolution and its comparison protocol are described in [solver-challenge.md](solver-challenge.md). No common trait or solver-plugin ABI is implemented yet.

## Unified authoring board

The board in `atelier/board.rs` shares selection (`panel` + named edge) with `atelier/edges_3d.rs`. It has one metric camera and persistent per-panel `layout_2d` / `layout_rotation_degrees` metadata. Board positions are `origin + rotation(local_uv)`. Layout does not modify material UV, placement, grain, masses or cache signatures. Shape edits inverse-rotate pointer coordinates into local anchors and controls; moving the local origin compensates both rotated 2D layout and 3D placement so untouched geometry stays fixed. `Command::BoardTransform` commits translation and angle atomically. The zero angle is omitted from serialization to preserve old physical hashes.

`atelier/workspace.rs` owns visual styling, the piece browser and format information. `atelier/seam_links.rs` draws sparse correspondence guides in either view. `CompiledSeam.guides` contains nine transient pairs of weighted boundary attachments derived during compilation from the same range/reversal/notch map as the physical seam. They do not enter energies, contacts or saved documents. Default overlays use three guides per seam, expanding the active pair to nine; dash counts are bounded for distant pieces.

The 2D board uses indexed vertices instead of duplicating every triangle corner. Boundary projection reuses one camera matrix per pass. Avatar visibility queries reuse `MeshCollider`'s static BVH and the existing two-sided triangle predicate; inclusive, padded slabs handle silhouettes and parallel rays. This is a picking optimization, not an OGC collision change. CPU equivalence tests and the standalone `picking_probe` benchmark cover it.

`garment/curves.rs` subdivides cubic controls with a fixed 0.25 mm geometric error bound, independent of triangulation density. `polygon.rs` validates that sampled contour and joins its temporary subsegments back into the original named boundary chain. Thus sewing quadrature still uses rest arclength on one semantic edge. `serde_json/float_roundtrip` preserves exact input floats on reload; small parsing changes can otherwise alter Delaunay decisions. The legacy template formats remain readable through optional serde fields.
