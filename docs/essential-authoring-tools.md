# Essential tools for a usable garment editor

Research and source-code audit: **15 October 2026**. Scope: the everyday authoring workflow of Marvelous Designer / CLO, compared with ClothCraft's current working tree. This is a product recommendation, not a claim of feature parity, native UI acceptance or newly implemented tools. No GPU work was performed.

## What “usable” should mean

For the first release, a new user should be able to make a T-shirt, add a pocket/apron, and make a simple dress entirely in the UI; correct mistakes; save; reopen; and export a predictable static mesh. This is our proposed acceptance target, not a completion claim. Reliable simulation and actionable failure reporting remain gates even when all editor tools exist.

Separate two outcomes:

- **3D garment authoring:** accurate pattern editing, assembly, arrangement, fitting, recoverable documents and static asset exchange.
- **Manufacturing-ready patternmaking:** additionally requires cutting versus sewing contours, seam allowances, notches/labels/grain, reliable physical-scale printing, apparel DXF and grading. These are essential to that outcome, but should not block the first useful 3D workflow.

The priority order below is our engineering judgment. Public manuals establish tool behavior; they do not prove which backlog order is best for ClothCraft. We have not conducted user interviews or timed usability sessions for this report.

## Minimum tool families and current gaps

“Present” below means code exists, often with CPU/headless checks. Several recent UI additions still await native/GPU acceptance. A data-model field is not counted as an available editing tool.

| Family | Minimum useful behavior | ClothCraft now | Next deliverable |
|---|---|---|---|
| Document lifecycle | New/Open/Save/Save As, dirty marker, recovery, asset relinking, separate export | Export writes fixed filenames; project reopening and avatar selection use the CLI | `SAVE-01`: transactional save/open dialogs; then recovery. Cancelled/failed saves must retain the old file |
| Selection and organization | Select pieces/points/edges, box selection, names/groups, hide/isolate, selection lock, undo/redo, copy/paste | Linked 2D/3D selection, 2D marquee, clipboard, browser and history exist; no complete hide/isolate/lock or 3D group editing workflow | Add presentation-only visibility/selection state before physical Freeze; make the active selection/tool unmistakable |
| Precise pattern drawing | Polygon/rectangle/ellipse, curved boundaries, numeric lengths/coordinates, snapping, split/remove/smooth points | Primitives, Bézier handles, coordinates, rectangle dimensions, target lengths and unsewn splits exist | Finish the edit loop: curve continuity tools, safe sewn-boundary edits and clear diagnostics. General polygon sizing is not equivalent to rectangle sizing |
| Pattern modification | Mirror with optional linked editing, offsets, cut/merge, trace boundaries into pieces | Independent mirrors, booleans/holes, internal paths and ruffle strips exist | Linked symmetry; measured offsets; Cut / Cut & Sew. A boolean or hole is not an internal cut |
| Sewing and seam correction | Whole-edge and partial sewing, ordered multi-edge sewing, edit/reverse/delete, length differences, correspondence markers | Whole named edges and whole internal paths can be sewn in either view; delete/undo/easing exist | `SEW-01`, then the first part of `SEW-02`: editable ranges and 1:N chains; then M:N and notch editing |
| Arrangement | Move/rotate/flip, front/back/sleeve placement, wrap around limbs, clearance feedback, reset selected pieces | Gizmos and numeric flat placement exist; moving a draped piece returns that piece to its flat arrangement | `ARR-01`: explicit body anchors and clearance preview; avoid silently changing rest shape or assuming every avatar has standard proportions |
| Draping controls | Play/pause/step, grab, persistent pins, freeze/unfreeze selected pieces, distinguish simulation states | Worker controls and mouse grab exist; model pins are limited and have no complete authoring UI | `SIM-01`: pin/unpin with clear targets; then Freeze with contact preserved and explicit state restoration |
| Measurement and fit | Curve length, seam-length comparison, material-space distances, body/garment measurements, strain legend | Boundary lengths, mass, global strain/seam metrics and a strain overlay exist | A ruler and persistent measurements, selectable diagnostic locations, initial-intersection feedback; do not advertise the strain overlay as measured pressure |
| Fabric and grain | Assign shared fabric, density, directional stretch/bending, visible editable grain per piece | Fabric presets/properties exist; grain affects the model but lacks a dedicated visual editing workflow | Grain arrow and numeric angle with coherent cache rebuilding; separate physical properties from appearance |
| Reuse and exchange | Small starter blocks, avatar import with unit preview, static garment export with predictable scale/groups/UVs | T-shirt/dress examples, CLI OBJ avatar input, static OBJ and SVG preview output; web generator links only | In-app avatar import and a small documented block library, then the static OBJ+MTL/GLB contracts. External mesh import does not reconstruct editable patterns |

Primary behavior references for drawing and modification: [curve-point editing][curve], [internal offsets][offset], [Cut / Cut & Sew][cut], [linked editing][linked]. For assembly and draping: [CLO 3D sewing][sew], [arrangement][arrange], [pins][pins], [Freeze][freeze]. Measurements and diagnostics: [2D measurements][measure], [fit maps][fit].

## The next implementation slices

Keep existing roadmap IDs. The additional IDs below are proposed work, not completed capabilities. Each slice needs a small fixture and undo/save/load coverage; no automatic GPU tests are implied.

| Order | Task / scope | Concrete acceptance case |
|---|---|---|
| 1 | `SAVE-01`: explicit document lifecycle, portable references and recovery | Save an edited shirt to a chosen folder, reopen it from another folder, relink a missing avatar, and recover after interrupted writing without corrupting the previous save |
| 2 | `SEW-01`: partial ranges and existing-seam editing in both views | Sew only 100 mm of a 400 mm edge; drag the endpoints, reverse the correspondence, delete/undo, and reload without affecting the unused 300 mm |
| 3 | `SEW-02a`: ordered 1:N sewing with length/ease feedback | Join a sleeve-cap chain to front and back armhole boundaries as one understandable relation. Show order and cumulative lengths; reject overlaps and ambiguous ordering |
| 4 | `SIM-01`: persistent world-space pins, followed by `SIM-02` Freeze | Hold a skirt waist while the hem drapes; remove the support and resume. Freeze an inner piece while it remains a collider. Explicitly define remesh and unfreeze velocity behavior |
| 5 | `ARR-01`: body-aware arrangement and preflight | Place front/back/sleeves outside a supported avatar, display problematic pieces before starting, and reset only the selected piece |
| 6 | `EDIT-01` and `MEASURE-01`: organization and measurement | Isolate a sleeve, lock the avatar against selection, measure its curved edge and a matching chain, then restore visibility without changing physics |
| 7 | `DRAW-04a/b`: measured offsets, then internal Cut / Cut & Sew | Generate a 10 mm inset on a curved panel with explicit corner/self-intersection handling; split along an internal path and optionally create its new seam, preserving area and references |
| 8 | `DRAW-03`: linked symmetry and unlink | Change one sleeve curve and update the other once; retain correct seam direction and grain; unlink and edit independently; undo the whole operation |
| 9 | `GRAIN-01`, `BLOCK-01` and static interchange | Rotate grain without rotating the board; load documented shirt/skirt/trouser blocks; import an avatar with explicit units and round-trip the static garment through the chosen interchange subset |

This order groups user-visible deliveries; it is not a dependency graph or calendar. Pin target semantics must be designed before using pins as the implementation of Freeze. Block/import work can be developed on CPU independently of GPU solver validation. Existing numeric editing should be polished, not reimplemented under new button names.

**Why partial and multi-edge sewing come early:** a useful editor must tolerate patterns whose segment boundaries differ. A sleeve and its armhole should not need manual topology surgery just to establish correspondence. Internal paths already allow some pocket/ruffle cases; they do not replace editable seam ranges or ordered chains. The manuals expose these as distinct sewing interactions. [CLO 3D sewing][sew]

## The next level after a simple top

- **Darts, pleats, fold arrangement and slash/spread:** useful for fitted dresses, shaping and controlled fullness. Start with one explicit dart or fold operation. A drawn internal path alone is neither a sewn dart nor a physical crease. [Dart][dart], [segment darts][segment-dart]
- **Tacking, controlled layering, elastic bands and fastening:** important for collars, cuffs, gathered waists and jackets. They require distinct physical semantics and suitable regression scenes; toggles that merely change a color are not implementations.
- **Reference image with scale calibration and tracing:** a useful CPU-first way to reproduce a lawful pattern source. Separate image tracing from vector import and from a parametric pattern generator. Record source/license for bundled patterns.
- **Appearance and DCC finishing:** basic fabric color/texture, UV inspection, thickness as an explicit export option, and material assignment. Retopology, detailed topstitching, zippers/buttons, cinematic rendering and animation are later specializations for this first milestone.

For actual sewing/manufacturing, promote **seam allowance, notches, grain/labels, physical-scale PDF/SVG and DXF** to release gates. Cutting and sewing contours must remain distinct; notch markers must have stable material positions. Test a printed 100 mm ruler and a curved piece with an allowance before claiming usable print output. Grading and size tables follow once single-size geometry and exchange are reliable. [CLO allowance properties][allowance], [CLO notches][notch], [existing interchange contracts](interoperability.md)

## UI organization without another wall of buttons

Keep the main rail to a small number of families: **Select/Edit**, **Draw**, **Internal/Modify**, **Sew**, **Arrange**, **Pin/Grab**, **Measure**. Variants belong in flyouts or contextual controls; retain direct access to frequently used tools such as Rectangle and Rotate. The radial menu provides fast access to the same commands rather than a separate implementation. These are proposed groups; absent tools should not appear as working controls.

Selecting a seam should expose its range, direction, lengths and Delete/Edit actions. Selecting a piece should expose fabric/grain and placement. Numerical input, mouse gestures and shortcuts must dispatch the same transactional commands. Keep camera/background controls in viewport headers and save/open at application level.

Use separate states with explicit meanings:

| State | Proposed ClothCraft meaning |
|---|---|
| Hidden | Not drawn; physics unchanged |
| Selection locked | Not selectable through normal picking; physics unchanged |
| Pinned | Specified material attachment held to a declared target |
| Frozen | Current pose held; remains a contact obstacle |
| Deactivated | Excluded from simulation/contact under an explicit policy, while the editable piece remains in the document |

The Freeze/Deactivate distinction is documented in Marvelous; the rest of this table is our proposed application contract. [Freeze/Unfreeze][freeze] Hiding a piece must never secretly turn off collisions. Unsupported backend capabilities need an explanation rather than a silent approximation.

## Code audit and boundaries

- `src/authoring.rs`: current transactional command enum covers drawing, edits, mirrors, booleans, sewing, fabric and clipboard; no dedicated pin, freeze, offset or cut command.
- `src/garment/seam.rs`: `BoundaryRef.range` and `SeamSpec.notches` provide lower-level foundations. They do **not** establish an interactive partial-sewing/notch tool or arbitrary M:N relation model.
- `src/garment/template.rs`: `grain_degrees` and `pin_points` exist. Current material pins must coincide with rest mesh vertices and compile to arrangement targets. Live pins on an already draped surface need an explicit target/attachment design, not just another checkbox.
- `src/atelier/board.rs`, `edges_3d.rs`, `authoring_ui.rs`, `drafting_ui.rs`: current selection/drawing/sewing adapters; preserve shared semantic IDs.
- `src/atelier.rs`, `workspace.rs`, `fitting.rs`: current export/reopening paths; do not confuse export with a complete document lifecycle.
- `src/render.rs`: the current strain visualization is not a calibrated fit or pressure estimator. Physical measurements need definitions, units and material evidence. [CLO distinguishes these map types][fit]

All new source and UI text stays English. Keep source curves/material coordinates authoritative and rebuild derived caches coherently. No solver change is required merely to add rulers, file dialogs or visibility controls. Pins, freeze, crease constraints and layer/contact behavior do require physical capability checks; solver independence is not permission to fake those operations.

## Evidence record

The linked official articles were consulted on 15 October 2026, through their accessible page text/search results; videos were not treated as watched. The [official CLO introductory guide][guide] provides an older toolbar taxonomy (2021); it is not evidence of the exact current UI. Current help articles supplied the specific behaviors above. Sources are paraphrased, no proprietary assets/code are incorporated, and priorities/acceptance fixtures are independently proposed.

[curve]: https://support.marvelousdesigner.com/hc/en-us/articles/47358435895065-Edit-Curve-Point
[offset]: https://support.marvelousdesigner.com/hc/en-us/articles/47358442871705-Offset-as-Internal-Line
[cut]: https://support.marvelousdesigner.com/hc/en-us/articles/47358433146393-Cut-Cut-Sew
[linked]: https://support.marvelousdesigner.com/hc/en-us/articles/47358366174233-Linked-Editing
[sew]: https://support.clo3d.com/hc/en-us/articles/360007863993-3D-Sewing
[arrange]: https://support.marvelousdesigner.com/hc/en-us/articles/47358262924185-Arrange-Pattern-with-Arrangement-Points-Flip-Wrap-Direction
[pins]: https://support.marvelousdesigner.com/hc/en-us/articles/47358146159769-Pins
[freeze]: https://support.marvelousdesigner.com/hc/en-us/articles/47358315135897-Freeze-Unfreeze
[measure]: https://support.marvelousdesigner.com/hc/en-us/articles/47358141416985-Show-2D-Measurements-ver-2025-0
[fit]: https://support.clo3d.com/hc/en-us/articles/360000436368-Garment-Fit-Maps
[dart]: https://support.marvelousdesigner.com/hc/en-us/articles/47358304858265-Dart
[segment-dart]: https://support.marvelousdesigner.com/hc/en-us/articles/47358357640345-Segment-Darts
[allowance]: https://support.clo3d.com/hc/en-us/articles/115013370408-Edit-Seam-Allowance-Properties
[notch]: https://support.clo3d.com/hc/en-us/articles/115015735467-Notch
[guide]: https://support.clo3d.com/hc/en-us/article_attachments/48293801440281
