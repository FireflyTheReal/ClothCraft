# ClothCraft roadmap

Build a useful garment authoring tool whose physics can evolve independently. Public Marvelous Designer manuals inform expected interactions; ClothCraft implements its own code, data model, UI and test fixtures. This is an independent implementation effort, not an affiliated product or a claim of formal clean-room certification.

Status: research prototype, October 2026. **Most of the product is still missing. A roadmap item is not a supported feature.** Read the [source/behavior ledger](docs/manual-study.md), [current authoring guide](docs/authoring.md) and [input/output capability matrix](docs/interoperability.md).

## Next milestones, in order

Each row is a contribution-sized track, not a promised release date. Finish one verifiable slice before adding another format or toolbar.

| Order | Outcome | Start in | Acceptance / evidence |
|---|---|---|---|
| P0.1 | Reliable save/open and portable projects | `fitting.rs`, `atelier.rs` | File picker, explicit save vs export, relative/relinked avatar paths, autosave recovery; reopen the same document/state after a forced shutdown |
| P0.2 | Understandable drawing and sewing | `atelier/board.rs`, `authoring.rs`, `garment/seam.rs` | Numeric dimensions, add/remove contour points, clear orientation and preflight intersections; a new user completes a top without editing JSON |
| P0.3 | Stable cloth at a measured realtime budget | `ogc/`, `vbd.rs`, `gpu_vbd.rs` | Shirt/apron/dress, supported and free cloth, multiple mesh densities; report step P50/P95, delivered physical time, seam gaps, stretch and intersections. Preserve accuracy checks |
| P0.4 | A workspace shell you can actually open | `editor.rs`, `render.rs`, `scripts/make-macos-prototype.sh` | `cargo build --features desktop` yields a double-clickable macOS bundle; menu bar, tool strip, left palette, 3D/2D window pair, tabbed inspector and status bar all respond; no fake tool buttons |
| P0.5 | The desktop feature is compiled on every push | `.github/workflows/cpu.yml` | A `cargo check --features desktop` job exists; a green CI run means the window builds, not just the CPU core |
| P0.6 | One language in the interface | `atelier.rs`, `atelier/*`, `editor.rs`, `bin/` | Every user-visible string and every contributor-facing guide is plain English; no tool label, tooltip, error or panel heading is left in another language |
| P0.7 | The capability register covers the whole bundle | `scripts/clo3d/extract_surface.py` | Every `libClo*.dylib` is symbol-scanned, not just the main executable; the register states its real coverage and no longer claims a partial scan is complete |
| P1.4 | Notches, internal lines and darts | `authoring.rs`, `garment/`, `atelier/board.rs` | The three smallest high-count gaps in the register (15 + 26 + 21 commands) become real semantic objects with stable ids, preserved through save/load and remeshing |
| P1.5 | A graded size run | pattern model + grading adapter | Grade rules attached to stable point identities, reversible, round-tripped; the register counts 26 grading commands and ClothCraft implements none |
| P2.3 | Measured material calibration | `fabrics.rs`, future material model | Nonlinear, directional stiffness fitted to measured data the project owns; no copied preset numerics. Evidence base: [fabric model evidence](docs/clo3d-fabric-model-evidence.md) |
| P1.1 | Practical static asset interchange | `avatar.rs`, `garment/export.rs`, future adapters | OBJ+MTL round trip, then static GLB; explicit units/axes, groups, normals and UVs; Blender and a second independent viewer agree |
| P1.2 | Industrial pattern exchange | canonical pattern model + new format adapters | DXF-AAMA and DXF-ASTM separately; import/export contours, grain and notches with independent fixtures; cutting vs sewing lines are explicit |
| P1.3 | Usable pattern printing | `garment/export.rs` | Physical SVG/PDF page dimensions, 100 mm ruler, tiling, labels and allowances; printed measurements agree within documented tolerance |
| P2.1 | Production animation exchange | future adapters + animation model | Fixed-topology Alembic cache; USD mesh/material/time samples; FBX avatar skinning/animation import; playback and unit/time tests in external DCCs |
| P2.2 | Truly replaceable solvers | `fitting.rs`, `solver.rs`, `vbd.rs` | One lifecycle/capability contract, two real adapters, shared scene suite; no fake backend selector |
| P3 | Manufacturing and richer appearance | pattern/gradation and material adapters | Named grading dialect, size rules and reversible grades; USDZ packaging; optional legacy caches and SBSAR baking |

The format priorities below remain product priorities: a **critical** animation format is still blocked by an animation/data-model milestone. Format names alone are not deliverables. Friction, layered cloth, calibrated materials, symmetry, holes and internal lines remain substantial research/engineering tasks.

## 1. Draw → sew → arrange → simulate

- [x] Unified metric 2D board, rectangle/pen creation with straight and cubic Bézier boundaries; constrained Delaunay meshing.
- [x] Linked 2D/3D panel/edge selection, duplicate/delete, whole-edge sewing from either view, direction and explicit easing.
- [x] Sparse colored seam links and endpoint ticks in 2D/3D, linked hover/selection and distinguishable pending previews.
- [x] World-axis translation/rotation gizmos, numeric transforms, transactional preview and undo/redo.
- [x] Direct 3D move/rotate tools (G/R), edge-on rotation rings, 15° snapping and 2D board rotation independent of physical rest/grain.
- [x] Save editable templates/projects and reopen through the CLI; preserve v1/v2 data compatibility.
- [x] Edit existing anchors and Bézier handles; pan/zoom, snapping and saved 2D layouts.
- [x] Garment-CAD workspace shell: menu bar, tool strip, left display palette, 3D + 2D window pair with docked title bars, tabbed right inspector, status bar.
- [ ] Exact dimension entry, holes, internal lines, multi-selection and symmetry.
- [ ] Partial/free sewing, notch editing, 1:N / M:N relationships and clearer 3D direction preview.
- [ ] Arrangement points, body-aware placement and useful preflight intersection feedback.
- [ ] File picker, multi-document projects, portable asset bundles and recovery/autosave.
- [ ] Seam allowance, grain annotations, graded sizes and print layouts.

Acceptance: an unfamiliar user can draft a simple top, join its intended boundaries, place it outside the avatar, simulate, save and reproduce the same editable garment. New formats must not store transient mesh indices as sewing references.

## 2. Physics as a replaceable module

- [x] Authoring and material-space model independent of egui/wgpu; CPU-only build.
- [x] CPU Newton reference; CPU VBD; hybrid GPU VBD/OGC; real window timing and force checks.
- [x] Assembled nodal mixed block-CSR operator with a 3×3 block-Jacobi preconditioner, recovered from CLO 2026.1.310 evidence and guarded against the matrix-free path.
- [ ] Common backend lifecycle/capability contract, exercised by two real adapters before exposing a new selector.
- [ ] Standard scene suite and machine-readable quality/cost comparisons for alternative solvers.
- [ ] Resident GPU contact, fewer transfers, dense scenes and animated avatars.
- [ ] Friction, layered garments, better seam closure and calibrated material response.

The challenge is **more accurate AND faster**, with disclosed tradeoffs. Keep authoring, import/export and benchmarks useful even if a future solver replaces today's engine. See [solver challenge](docs/solver-challenge.md).

## Interoperability

The priorities below are the product requirements. Within the critical group, implement small, validated slices instead of advertising a full format after parsing one example. Import and export are separate capabilities.

| Format | Industry use | Priority | Current support / next milestone |
|---|---|---|---|
| DXF-AAMA / DXF-ASTM | Industrial sewing pattern exchange | **Critical** | Planned. Layers, units, contours, notches, grain and piece IDs; independent AAMA/ASTM fixtures and explicit unsupported-entity diagnostics |
| OBJ + MTL | Static garments, avatars, Blender, Maya | **Critical** | **Partial:** avatar OBJ import and garment OBJ export; no MTL/material round trip. Add groups, material assignments and documented axes/UV conventions |
| glTF / GLB | Modern 3D assets, PBR, realtime visualization | **Critical** | Planned. Static garment/avatar round trip first, then materials, textures and animation capabilities |
| Alembic `.abc` | Animated simulation caches for VFX/cinematics | **Critical** | Planned. Fixed-topology position/normal cache, timestamps, units and DCC playback fixture |
| OpenUSD `.usd/.usda/.usdc` | Production pipelines and Chaos Cloth workflows | **Critical** | Planned. Mesh/material/time-sample interchange; validate any Chaos-specific schema separately |
| FBX `.fbx` | Rigged characters, animation, Unreal, Unity, DCC | **High** | Planned. Avatar hierarchy, skinning, bind transforms and sampled animation; select dependency/distribution strategy before integrating |
| PDF / SVG | Print and share 2D patterns | **High** | **Partial:** preview SVG export only. No scale-accurate print/PDF yet; add physical page size, scale marks, tiling and seam allowances |
| Grading rule files | S/M/L/XL size ranges, industrial manufacturing | **High** | Planned. Identify concrete dialects; explicit units, size tables, stable point identities and reversible grade application |
| USDZ `.usdz` | AR and product presentation | **Medium** | Planned after USD: packaging, material/texture validation and viewer fixture |
| MDD / PC2 | Legacy deformation-cache pipelines | **Secondary** | Planned. Fixed vertex order, frame/time conversion and topology mismatch rejection |
| SBSAR | Procedural Substance materials | **Secondary** | Planned. Dependency/runtime evaluation and material baking workflow; appearance parameters are not calibrated cloth physics |

Every adapter must declare supported entities, units, axes/handedness, topology policy, material mapping and losses. Reject or report unsupported data instead of silently flattening it. Put adapters around the canonical garment model, not inside a solver. Add round-trip tests and open-source or self-authored fixtures with provenance. **Input and output have separate acceptance criteria** in [the interchange plan](docs/interoperability.md). Native CLO/Marvelous project files are not supported; do not infer editable garment support from mesh import.

## 3. Concrete authoring backlog from the manual study

The nine articles reviewed in the [dated manual ledger](docs/manual-study.md#authoring-backlog-review-2026-10-14) inform the tasks below. **Every task in this table is planned, not implemented.** Existing support for whole-edge seams, curves, numeric 3D placement and static pins in the model is not the complete editor workflow described here. These are independent behavior specifications, not a plan to copy proprietary code or file formats.

| ID / milestone | Small deliverable | Dependencies / starting point | Done when |
|---|---|---|---|
| SAVE-01 / P0.1 | Open, Save As, dirty-state feedback and portable avatar references | `fitting.rs`, `atelier.rs`; foundation for every later task | A saved shirt reopens from another directory with matching material/rest/state; missing avatar gives relink UI; cancelled or failed writes preserve the previous file |
| DRAW-01 / P0.2 | Rectangle width/height and numeric anchor movement in mm | `authoring.rs`, `atelier/board.rs`; current metric model | A 400 × 600 mm rectangle keeps exact dimensions through save/load; invalid values are atomic; one edit is one undo entry |
| DRAW-02 / P0.2 | Add/remove a contour point; split by arclength, ratio or count | DRAW-01; named-edge migration in `garment/curves.rs` and `authoring.rs`; MD-01 | Split a sewn Bézier without changing its outline; migrate seam ranges or reject explicitly; no dangling references after undo/remesh |
| SEW-01 / P0.2 | Interactive partial-edge sewing in both views with numeric length and endpoint handles | Existing `BoundaryRef.range` / compiler correspondence; `board.rs`, `edges_3d.rs`; MD-02 | Sew 100 mm of a 400 mm edge to another piece, reverse/edit/delete it, undo and reload; selected subranges and preview match the physical seam |
| ARR-01 / P0.2 | Named body arrangement anchors with preview, normal direction and clearance diagnostics | `avatar.rs`, `authoring.rs`, `gizmo.rs`; MD-06 | Place front/back/sleeve panels on an arbitrary supported avatar; show offending pieces before simulation; retain 2D rest data and undo the placement |
| SIM-01 / P0.3 | Edit persistent pins from 2D/3D and distinguish them from temporary mouse grabbing | Material-coordinate attachment model + solver capabilities; MD-07 | Pin/unpin a patch and resume; report what survives remeshing; no persisted triangle indices, hidden forces or silent conversion between hard and soft constraints |
| DRAW-03 / P1 authoring | Mirror copies with optional linked outline/sewing edits, plus unlink | DRAW-02; explicit relation model; MD-03 | Edit one sleeve and update its counterpart exactly once; mirrored winding, grain and seam orientation remain valid; unlink and undo are deterministic |
| DRAW-04 / P1 authoring | Internal lines, offsets and holes as distinct semantic objects | DRAW-02; topology/model migration and constrained mesher; MD-04 | A 10 mm internal offset remains attached to its piece; reject self-crossing offsets; a hole removes area/mass while a guide line does not |
| SEW-02 / P1 authoring | 1:N, then M:N ordered sewing chains and editable notch correspondences | SEW-01; DRAW-04 for internal-line sewing; MD-05 | Sew two short edges to one long edge with visible ordering and explicit ease; cancel partial input; preserve totals, orientation and notches after remeshing |
| ARR-02 / P1 authoring | Fold placement preview about an internal line | DRAW-04 + ARR-01; MD-08 | Fold a cuff/hem and undo it without changing material rest; preflight initial intersections; distinguish placement from a physical crease constraint |

Recommended first contribution: **SAVE-01**, then **DRAW-01**. Keep DRAW-02 and SEW-01 in separate reviewable changes. ARR-01 and physical performance work can proceed independently once they share the same document transactions. Do not block a usable save/draw/sew loop on automatic sewing, animation or visual trim tools.

### Release evidence, not feature counts

- A new user completes **T-shirt → added apron → simple dress** using only the UI, saves, closes and reloads each garment. Record the actual action sequence and any manual recovery.
- Every topology edit checks area/mass, stable semantic references, undo/redo and compatibility with old documents. Future schema changes need explicit migrations; do not silently change existing v1/v2 interpretation.
- For each physical backend, publish identical fixtures, settings and material data with seam gap, stretch, penetration diagnostics, step P50/P95 and simulated/wall time. Target >=0.95× physical time and >=55 UI frames/s on a named reference machine; this is an acceptance target, **not current universal performance**.
- Every import/export adapter must meet the separate direction-specific [interchange contract](docs/interoperability.md). Preserve the critical/high priorities above; manufacturing semantics and animation prerequisites determine implementation order.

## 4. Public-manual study queue

Next reading: darts/pleats, seam allowance, topstitch vs structural sewing, zippers/buttons, fabric testing/calibration, measurement, animation, retopology and UV/export details. Read each family before writing its implementation specification. Automatic sewing is later work, after explicit sewing and arrangement are reliable; generic AI automation is not a substitute for a correct garment model. Track observations, independent specifications, implementation status and acceptance cases in [manual-study.md](docs/manual-study.md). The full manual has not been exhaustively reviewed.

## 5. Desktop shell and the macOS prototype

Two desktop applications share the library:

| Binary | Module | What it is |
|---|---|---|
| `cloth-fit --view` | [src/atelier.rs](src/atelier.rs) | The interactive authoring atelier: draw, sew, arrange, simulate, export. **This is the app worth testing.** |
| `marvelous-cloth` | [src/editor.rs](src/editor.rs) | The older single-panel FEM reference editor. |

Both are **presentation only** in their UI layer: they reuse the same solver workers, camera interaction and file I/O, and add no control that does not do something. Both use a light workspace chrome with a blue accent. The atelier's 2D pattern board is a white drafting sheet with a light grid, dark piece names and an amber rotation handle; the 3D window uses a light studio backdrop ([render.rs](src/render.rs)).

Stated plainly, as limits rather than claims:

- **The `desktop` feature was not covered by CI.** `.github/workflows/cpu.yml` builds `--no-default-features`, so `editor.rs`, `render.rs` and `atelier.rs` were never compiled on a push, and **a green CI run did not mean the window built**. A `desktop` job now runs `cargo check`, `clippy --all-targets -- -D warnings` and the desktop test target with the feature enabled, and it is **gating** — the Linux runner installs the X11/Wayland/GL dependencies the test target needs in order to link, and the three GPU tests stay `#[ignore]`d so the job remains GPU-free. A separate `build` workflow compiles and uploads release binaries for **macOS, Linux and Windows** on every push (milestone **P0.5**, closed).
- The macOS bundle is a **hand-built prototype**, not packaging: no notarization, no installer, no update path, no signing identity. Only the dev profile is built, and no performance claim is attached to it.
- Docking is **fixed**. Panels resize but cannot be rearranged, torn off or saved as layouts, and the atelier still has no multi-document workspace.
- The displayed physical quantities come from the existing solvers. Nothing in the shell implies a capability the solvers do not have.
- The UI has **no automated visual regression test**. `cargo run --features desktop --bin cloth-fit -- --view --ui-smoke --output <dir>` writes `<dir>/ui.png`, and every colour change here was reviewed on those captures by eye. A pixel-diff baseline would be a real improvement, and is not done.
- The committed screenshots are **historical evidence, not a mirror of the current theme**. `docs/images/atelier.png` is cited by [publication validation](docs/publication-validation.md) and [turbo](docs/turbo-perf.md) for one specific measured run, so it is deliberately left untouched; `docs/images/atelier-light-theme.png` shows the current light workspace. `authoring.png` and `pattern-board.png` still depict the previous dark theme and should be regenerated from their own runs rather than replaced.
- No CLO3D icon, screenshot, font, artwork or pixel-level control is copied. The light chrome, accent colour, grid, cotation marks and title strips are original to this project.

Reproduce:

```sh
scripts/make-macos-prototype.sh                        # -> prototype/mac/ClothCraft.app (launches the atelier)
cargo run --features desktop --bin cloth-fit -- --view  # the atelier in place
cargo run --features desktop                           # the legacy FEM editor
```

The generated `prototype/` directory is not committed; rebuild it from the script.

## 6. Reverse-engineering inputs

These are not product features. They are the evidence base that decides what to
build next, and they are kept in the repository so that prioritisation does not
depend on memory or on someone's summary.

| Artefact | What it gives you |
|---|---|
| [CLO3D capability register](docs/clo3d-capability-register.md) | Every command slot, screen class, preset file and file extension recovered from the shipped CLO 2026.1.310, mapped topic by topic onto ClothCraft's actual state. |
| [`data/clo3d/`](data/clo3d/README.md) | The machine-readable form: `actions.json`, `screens.json`, `libraries.json`, `presets.json`, `resources.json`, `tool_icons.json` and a generated `ACTIONS.md`. |
| [`scripts/clo3d/extract_surface.py`](scripts/clo3d/extract_surface.py) | The extractor. Re-run it against a newer CLO and diff the register. |
| [CLO3D reverse-engineering dossier](docs/clo3d-reverse-engineering.md) | The solver: how the implicit timestep operator and its block-Jacobi preconditioner were recovered and what was implemented from them. |
| [Fabric model evidence](docs/clo3d-fabric-model-evidence.md) | What REA/Ghidra recovered about the material model, with Evidence IDs, as the base for P2.3. |
| Earlier-pass documents | [Architecture](docs/clo3d-architecture-earlier-pass.md), [feature catalogue](docs/clo3d-feature-catalog-earlier-pass.md), [format notes](docs/clo3d-format-notes-earlier-pass.md), [prior art](docs/clo3d-prior-art-earlier-pass.md). |

Read the register before proposing a new format, tool or panel: it already
contains the count, the owning viewport and the likely name of the thing you are
about to design from scratch. Two limits are worth repeating — the topic
assignment is a name heuristic, and a recovered name is a lead to decompile,
never a specification to implement.

## 7. Where to go next, in dependency order

The register changed the shape of the plan, so the sequence below is stated
explicitly rather than left implicit in the milestone table.

1. **One language, one register.** Finish the English pass (P0.6) and widen the
   register to every library (P0.7). Both are cheap, and both remove a source of
   confusion that costs more than it saves: a half-translated interface, and a
   register whose coverage is narrower than it looks.

   **Outstanding from the first English pass.** The application UI, the
   contributor guides (`AGENTS.md`, `architecture.md`, `handoff.md`,
   `authoring.md`, `interoperability.md`) and the user-visible README are done.
   The registers and dossiers are English by construction. What remains is ten
   inherited French documents, none of which describes the current interface
   directly: `docs/ogc.md`, `docs/research.md`, `docs/validation.md`,
   `docs/first-fit.md`, `docs/jitter.md`, `docs/turbo-perf.md`,
   `docs/template-format.md`, `docs/research-patterns.md`,
   `docs/ogc-validation.md`, `docs/meshing.md`. Translate them when they are next
   touched for content, rather than in a separate sweep, so that the translation
   is checked against the code at the same time.

   **The language coupling is a real maintenance cost.** Several documents quote
   atelier control labels in bold. Every UI string rename therefore invalidates
   the documents that quote it, and a stale quote is worse than a French one
   because it looks authoritative. Prefer describing a control by what it does
   over quoting its current label, and when a label must be quoted, verify it
   against `src/atelier*.rs` rather than against another document.
2. **The document model is the bottleneck, not the solver.** The five largest
   uncontested gaps in the register — notches (15), internal lines (26),
   darts/pleats (21), grading (26) and animation (20) — total 108 commands, and
   **none of them needs new physics**. Every one is blocked on the same missing
   thing: a document model whose objects have stable semantic identities that
   survive save/load and remeshing. That is DRAW-02/03/04 and SEW-02, and it is
   the highest-leverage work in the repository.
3. **Then the smallest real wins.** DRAW-01 (numeric dimensions) is the first
   slice; P1.4 turns notches, internal lines and darts into first-class objects;
   P1.5 adds a graded size run. Each is independently reviewable and each has a
   count behind it.
4. **Calibration last, and measured.** P2.3 only becomes honest once there is
   data the project owns. The register shows 1,579 shipped preset files: their
   *existence* tells us the category matters, their *contents* are proprietary
   and must not be copied. [The fabric evidence pass](docs/clo3d-fabric-model-evidence.md)
   exists to describe the mechanics to reimplement, not the numbers to reuse.
5. **Keep the evidence honest.** Re-run the extractor after any CLO upgrade and
   diff; treat a change in the register as a prompt to re-open the related
   roadmap item, and treat a green CI run as evidence only about what CI builds.
