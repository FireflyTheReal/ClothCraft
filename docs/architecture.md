# Architecture and invariants

The CPU-only [solver consistency experiments](solver-consistency.md) live in
`solver_lab`. Their matrix-free coupled solve shares the exact material factors
and existing OGC guards. They are not wired into the editor or GPU worker.

The repository remains one Rust package with three executables. CPU numerical modules do not import eframe or wgpu. The `desktop` feature adds rendering, the `gpu_vbd` compute backend and interfaces; disable it to build only the CPU core.

## Data flow

`solver_lab::Backend` is an opt-in CPU research adapter over `CompiledGarment` and `State`, with working VBD, XPBD/OGC and finite-stiffness AVBD paths. Construction validates the initial collision state; a backend switch starts fresh caches. It is not a plugin ABI or an editor setting. The native Box3D harness exchanges files and requires a separate surface audit because its sphere lattice has different collision coverage. See [solver-lab.md](solver-lab.md).

The [EE transition experiment](edge-transition.md) is a contact-law variant exercised through the existing CPU VBD, not a solver backend. `OgcWorld::experimental_interior_edges` defaults to false; GPU stepping rejects it. Its extra terms never replace geometric movement bounds. `VbdSolver::contact_refresh_audit` optionally records within-step feature-refresh energy deltas without changing acceptance or prediction.

The separate [finite-primitive ablation](primitive-contact.md) recomputes
finite distances during local solves and retains a force halo beyond the
unchanged geometric regions. It also defaults off, is CPU-only and is mutually
exclusive with the EE taper. Its planar sliding test demonstrates spurious
tangential forces, so zero refresh jumps do not qualify it for production.

The VBD contact region now combines residual OGC balls with sparse verified
separating half-spaces for very close primitive pairs. `ogc/directional.rs`
constructs these regions on CPU; `vbd/local_step.rs` clips local steps without
moving their detection centers. The hybrid path keeps directional vertices in
its sparse f64 CPU solve. Other GPU kernels and constitutive energies are
unchanged. Read [the proof, tests and headless lab](directional-contact.md).

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

The legacy editor uses the single-panel `Project`. `cloth-fit` uses `FittingProject`: a multi-panel template, avatar, settings and signed position cache. The CLI and interface consume the same `FittingScene` without duplicating the solver.

The current `cloth-fit` path uses `VbdSolver` + `OgcWorld`, or `GpuVbd::step` with the same OGC topology and energies. The Newton diagram above describes `--reference`. The workspace worker receives a generation number: a result computed on an old mesh never replaces the current scene. It owns the physical state and fixed-step clock independently of egui refresh. Only one output snapshot is retained; control updates never reinsert an old displayed state. GPU resources and material terms are reused, and the OGC candidate cache never replaces distance/bound recomputation. See [Turbo](turbo-perf.md).

The [coupled seam refinement experiment](seam-block-refinement.md) is a CPU numerical module, disabled by default on both CPU and hybrid paths. It is enabled only by the research API/example, not garment settings or UI. Components derive from compiled sewing coefficients; additional steps must decrease the unchanged objective inside the original OGC balls. Current measurements reject production activation.

`vbd/local_step.rs` also exposes an opt-in CPU `set_local_solve_probe` for
backtracking depth and testing an unshifted positive-definite Hessian block.
Defaults retain the existing numerical path; there is no document field or UI
control. `examples/settling_zoo.rs` owns fixtures, measurement thresholds and
restart/release protocols. See [its evidence and limitations](settling-zoo.md).

## Responsibilities

| Module | Role | Input / output |
|---|---|---|
| `authoring.rs` | Transactions, history and rigid placements | Template + command → validated template + compiled model |
| `authoring/clipboard.rs`, `atelier/selection.rs`, `atelier/radial.rs` | Semantic clipboard, transient selection and tool menu | Stable panels/boundaries → commands; no persistent mesh indices |
| `fabrics.rs`, `atelier/fabrics_ui.rs` | Documented presets, extension-under-load conversion and controls | SI properties → `Command::Fabric` compilation, without changing rest geometry |
| `atelier/authoring_ui.rs`, `atelier/gizmo.rs` | Drawing, sewing, selection and gizmos | Interactions → commands; previews do not remesh every frame |
| `model.rs` | Rectangular rest geometry and FEM mesh | `Pattern` → `Mesh`, mass, hinges |
| `garment/template.rs` | Editing model, validation and compilation | `GarmentTemplate` → `CompiledGarment` |
| `garment/polygon.rs`, `garment/meshing.rs` | Validated contours, constrained Delaunay and meshing metric | Points/boundaries → UVs and triangles, quality checks |
| `garment/seam.rs` | Boundary correspondence and sewing energy | Rest lengths → quadrature, elements |
| `garment/embedding.rs` | Geometric attachments in pattern coordinates | `MaterialPath` → transient barycentric coordinates |
| `garment/export.rs` | CPU artifacts independent of UI state | Per-panel OBJ, SVG, fiber directions |
| `math.rs`, `energy.rs` | Second-order AD, membrane and bending | Current + rest geometry → energy/derivatives |
| `contact.rs` | Vertex/sphere penalty | Discrete contact, per-vertex thickness |
| `avatar.rs` | OBJ, static BVH and local signed distance | Mannequin → `Collider` implementation |
| `fitting.rs` | Arrangement, cache and transactional steps | FittingProject → FittingScene |
| `solver.rs` | Implicit Euler, Newton/PCG, Armijo | Compiled model + state → state/metrics |
| `project.rs` | Legacy single-panel scene format | `Project` JSON v1 |
| `bin/cloth-template.rs` | Headless CLI | example / validate / bake / simulate |
| `editor.rs`, `render.rs` | Interface and display | Consumers of the CPU model |
| `ogc/` | Feasible regions, VF/EE, barriers and bounds | Topology + positions → contacts + conservative radii |
| `ogc/simd.rs` | Runtime-dispatched f64 edge-pair geometry | Four independent pairs per AVX packet; scalar oracle/fallback, unchanged current distances and movement bounds; [validation](cpu-contacts-and-jitter.md) |
| `ogc/incidence.rs`, `ogc/canonical.rs` | Compact contact rows and canonical integer-key sorting | same contact identities/order; unrestricted fallback for large ID ranges; no persistent mesh indices |
| `vbd.rs` | Vertex-block implicit Euler, coloring including contacts | Same FEM/sewing energies, f64 reference |
| `vbd/prediction.rs` | CPU/GPU adaptive initialization, accepted-step history | Velocities/previous acceleration → initial estimate, unchanged inertial target |
| `motion.rs` | Accepted-step oscillation diagnostics on CPU and GPU | bounded semantic hotspot report, no change to state or solver; reset on generation changes |
| `vbd/audit.rs` | Explicit offline energy and accepted-step accounting | read-only f64 terms, frozen-contact balance and final residual; no automatic runtime cost; [scope](energy-and-release.md) |
| `vbd/coloring.rs` | Deterministic greedy coloring and dependency-preserving precision levels | same colors/vertex order with a small-palette mask; unrestricted fallback; only independent f64 minimizations run together |
| `garment/diagnostics.rs` | Stretch measurements independent of rendering | Material rest + state → area-weighted percentile and maximum |
| `gpu_vbd.rs`, `vbd.wgsl` | Local f32 solve, exact gradients, approximate positive Hessians | CPU candidates/bounds → GPU compute → validated state |
| `ogc/filter.rs`, `gpu_vbd/contact_filter/` | Conservative GPU BVH refit/search and separating-axis rejection | current positions → candidate IDs → unchanged f64 contact geometry/bounds; [contract](gpu-contact-search.md) |
| `picking.rs` | Rays and barycentric attachments | Mouse → grab force, no teleportation |
| `bin/cloth-fit.rs`, `atelier.rs`, `gpu_bench.rs` | CLI, workspace and rendering measurements | Projects / reports / opt-in captures |

## Contracts to preserve

**Rest geometry.** `Mesh.uv` is local to each panel and expressed in metres, without an atlas or [0,1] normalization. `Triangle.rest`, `inv_dm` and `area` derive only from the pattern. Rigid placements do not affect these values or masses.

**Materials.** The template separates `FabricProperties` from `Panel.grain_degrees`. Compilation creates an oriented `Material` per panel. Triangles and hinges reference it; `vertex_material` supplies contact thickness. Density is already integrated into mass: rebuild the model when it changes, rather than updating only the compiled material array.

**Topology.** Panels retain distinct vertices at seams. Vertex/triangle ranges identify each panel. The compiler rejects overlapping seam intervals on the same boundary; a shared corner is allowed. Multilayer sewing needs a richer relationship model.

**Newton reference.** Both APIs enter the same private `Problem`. Materials, inertia, gravity, damping, contacts and sewing are assembled together. Fixed constraints eliminate their DOFs. Errors preserve the previous state; reaching the iteration budget returns a state with `converged=false`. Seam metrics supplement, rather than replace, residuals.

**VBD/OGC.** The same energies are minimized by vertex blocks. Coloring includes all dependencies from elements, contacts and grabs. Bound centers change only at a new detection; local proposals are clipped and then checked against energy. The f32 GPU backend validates results and bounds before modifying `State`. A fixed budget does not establish convergence. CLI VBD settings are written to `run.json`; `FittingProject.solver` retains historical Newton settings.

Adaptive initialization keeps accepted-state history in `VbdSolver`, shared by CPU/GPU paths. It is not part of editable files; reconstruction or a different external state invalidates it. Damping-corrected acceleration chooses only the initial solver estimate; the objective retains full gravity. See [the artificial-stretch diagnosis](adaptive-initialization.md). The default budget has been 16 iterations since that correction.

**Attachments.** `MaterialPath` stores a panel ID and 2D points. `embed_path` splits segments at triangle crossings before barycentric interpolation, so a guide follows folds rather than crossing the space between its endpoints. `EmbeddedPoint` values are caches to rebuild after remeshing; they do not automatically detect a different mesh of equal size. The current search scans triangles and suits small patterns; a spatial index can replace it.

**Units.** Length m, mass kg, time s, force N, energy J, density kg/m², membrane N/m, bending N·m, sewing coefficient N/m². Warp/weft guides are not an explicit yarn model.

## Localized extensions

Since the [earlier contact activation change](jitter.md), `ContactSet.contacts` also contains nearby feasible interactions whose energy is still zero: force activation remains at `r`, while selection extends to `rq`. Coloring and incidence include these dormant terms. `active_contacts` / `VbdMetrics.contacts` count positive-energy terms at detection; `contact_terms` describes retained-term cost. Canonical IDs are sorted/deduplicated before coloring. The GPU works in a translated coordinate frame, restored on readback; CPU state/contacts remain global f64. Account for this frame when adding targets or GPU diagnostics. Residual jitter remains unresolved.

1. Extend current straight-edged holes to Bézier contours, then add spatial resolution control.
2. Transfer state during dynamic remeshing; explicit remeshing currently reconstructs the initial arrangement.
3. Extend current editing to dedicated hole-point tools and robust migration of referenced contour edits. Numeric point movement and partial sewing already exist.
4. Complete the GPU contact pipeline beyond the implemented BVH refit/search: exact geometry, feasible regions and ball construction still run on CPU. Add friction and dedicated seam treatment. A seam never excludes all collisions between its panels; OGC preserves separated states rather than repairing initial intersections.
5. Develop constitutive laws from measurements or homogenization. Preserve material coordinates and objectivity tests; do not invent calibration.

The backend evolution goal and comparison protocol are described in [solver-challenge.md](solver-challenge.md). There is no common production solver trait or plugin ABI yet.

## Unified authoring board

The CPU drafting extension is documented in [drafting-tools.md](drafting-tools.md). `garment/domain.rs` adds optional hole contours and internal material paths; `shapes.rs` supplies primitives/mirrors/booleans, while `authoring/geometry.rs` migrates semantic references transactionally. Internal paths are CDT constraints with material on both sides, exposed as named chains to the existing seam compiler. No solver-specific drawing or new GPU seam representation is required. `atelier/drafting_ui.rs` only adapts these operations to controls.

The board in `atelier/board.rs` shares selection (`panel` + named edge) with `atelier/edges_3d.rs`. It has one metric camera and persistent per-panel `layout_2d` / `layout_rotation_degrees` metadata. Board positions are `origin + rotation(local_uv)`. Layout does not modify material UV, placement, grain, masses or cache signatures. Shape edits inverse-rotate pointer coordinates into local anchors and controls; moving the local origin compensates both rotated 2D layout and 3D placement so untouched geometry stays fixed. `Command::BoardTransform` commits translation and angle atomically. The zero angle is omitted from serialization to preserve old physical hashes.

`atelier/workspace.rs` owns visual styling, the piece browser and format information. `atelier/seam_links.rs` draws sparse correspondence guides in either view. `CompiledSeam.guides` contains nine transient pairs of weighted boundary attachments derived during compilation from the same range/reversal/notch map as the physical seam. They do not enter energies, contacts or saved documents. Default overlays use three guides per seam, expanding the active pair to nine; dash counts are bounded for distant pieces.

The 2D board uses indexed vertices instead of duplicating every triangle corner. Boundary projection reuses one camera matrix per pass. Avatar visibility queries reuse `MeshCollider`'s static BVH and the existing two-sided triangle predicate; inclusive, padded slabs handle silhouettes and parallel rays. This is a picking optimization, not an OGC collision change. CPU equivalence tests and the standalone `picking_probe` benchmark cover it.

`garment/curves.rs` subdivides cubic controls with a fixed 0.25 mm geometric error bound, independent of triangulation density. `polygon.rs` validates that sampled contour and joins its temporary subsegments back into the original named boundary chain. Thus sewing quadrature still uses rest arclength on one semantic edge. `serde_json/float_roundtrip` preserves exact input floats on reload; small parsing changes can otherwise alter Delaunay decisions. The legacy template formats remain readable through optional serde fields.

## Point tools and view controls (Firefly integration)

See [integration provenance and fixes](firefly-integration.md). Polygon `target_lengths` is optional nonphysical metadata, omitted when empty and excluded from fitting cache signatures as well as `same_physics`. An explicit target command may change rest geometry; loading never enforces a stored target. Contour splits preserve cubics by de Casteljau subdivision and reject referenced seams/targets rather than migrating them implicitly. Existing holes and internal lines continue through the same domain mesher.

Board view rotation is radians; panel layout rotation remains degrees. Pointer and nudge adapters convert explicitly. Multi-point movement translates attached controls and invalidates transient selections when contour identity/order changes. Camera presets and orthographic projection share the same matrix with picking. Preferences separate immediately applied theme/snap from next-launch simulation/overlay defaults. The worker and physical solver paths are unchanged.

## Document and sewing extensions (16 October 2026)

`document.rs` owns portable avatar assets and atomic document replacement. `atelier/documents_ui.rs` handles dialogs, dirty state and recovery. `SeamSpec` keeps legacy A/B first segments plus optional ordered tails; `BoundaryRef.reversed` controls each trimmed segment. Cumulative arclength paths compile into the same quadrature/energy API, including both sides of discontinuous joints. Authoring, clipboard, mirrors, ownership checks and export traverse all references. See [contracts and limits](documents-and-sewing.md).

### Paired editing and checkpoint measurements (19 October)

`GarmentTemplate.mirror_links` describes bidirectional pairs by panel ID.
`authoring/linked.rs` synchronizes geometry inside the existing transaction;
there is no solver constraint or physical cache contribution from the link itself.
See [panel tools](panel-tools.md). [Draped checkpoints](dress-checkpoint-and-jitter.md)
preserve x/v/time/settings but rebuild predictor/contact caches on loading;
component motion and restart transients must be reported separately.
