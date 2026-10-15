# CLO3D capability register and ClothCraft comparison

**Purpose.** You cannot prioritise a reimplementation against a product you have
not enumerated. This document is the entry point to a machine-readable register
of CLO3D's user-facing surface, and the comparison against what ClothCraft
actually implements today.

**Target.** CLO `2026.1.310`, main binary `CLO_Standalone_OnlineAuth`
(126,231,984 bytes, universal), analysed through the thinned `arm64` slice
SHA-256 `b2d545d7baaefa3b303599c2bb51f49b94b04607531d55ba53f5693e9bff128f`,
50,164 symbol-table entries. The 21 `libClo*` libraries in
`Contents/Frameworks` are inventoried from their own thinned `arm64` slices:
90,899 `nm` entries (82,778 defined, 6,965 undefined, 1,156 debug/stab records).
That is **141,063** symbol-table entries for the whole bundle.

**Reproduce.** `python3 scripts/clo3d/extract_surface.py` — reads the shipped
application, writes [../data/clo3d/](../data/clo3d/README.md). Nothing executes
CLO or copies its assets.

## Evidence discipline

| Tag | Meaning |
|---|---|
| **Observed** | Read from the shipped artifact: symbol names, resource names, preset file names, extension counts. |
| **Derived** | A count, grouping or conclusion computed from observations. |
| **Inferred** | A reading of what a name probably does. Never load-bearing here. |
| **Unknown** | Not established. |

Three things this register deliberately does **not** claim:

1. **Topics are a name heuristic.** `OnAutoSewing` is filed under sewing because
   of its name. The classifier is in `TOPIC_RULES` and is easy to correct.
2. **A name is not a specification.** `OnAddDartFullness` proves a command
   exists. It says nothing about parameters, defaults, cost, or where the button
   sits.
3. **The action list is the command surface, not the UI tree.** It is recovered
   from symbol tables, so it will contain entries that are internal handlers
   rather than visible menu items, and it cannot tell you which are hidden
   behind a preference.

## A. Surface totals (**Observed**)

| Register | Count |
|---|---|
| Command slots (`View2DActionSlots` 407, `View3DActionSlots` 350, `MainMenuActionSlots` 281) | **1,038** |
| Screen classes (`Dialog*` 209, `View*` 12, `DockWidget*` 2, `Editor*` 1, `Widget*` 1) | **225** |
| Qt resource names in `Resources_Light.rcc` (1,150 SVG, 205 PNG, 6 XML, 5 Python, 3 ICO, 3 ICNS) | **1,393** |
| Shipped preset library files across 22 categories | **1,579** |
| Distinct file extensions under `Contents/` (frameworks excluded) | **71** |
| Symbols across the 21 `libClo*` libraries (`nm -a` on `arm64` slices) | **90,899** |
| Whole-bundle symbol-table entries (main executable + libraries) | **141,063** |

The full action list, grouped by view and topic, is
[../data/clo3d/ACTIONS.md](../data/clo3d/ACTIONS.md). Per-entry data is
[`actions.json`](../data/clo3d/actions.json).

### Library symbol inventory (**Observed**)

All 21 `libClo*` libraries in `Contents/Frameworks` — case-insensitive, so
`libCLOAPIInterface` is included — read with `nm -a` on each thinned `arm64`
slice and demangled with `c++filt`:

| Library | `nm` entries | Defined | Undefined | Debug | Top engine class |
|---|---:|---:|---:|---:|---|
| `libCloScene.dylib` | 78,098 | 73,559 | 4,538 | 1 | `PatternEditor` (1666) |
| `libCloWrapper.dylib` | 2,446 | 1,776 | 669 | 1 | `cwstring` (159) |
| `libCloSimulateMetal.dylib` | 2,377 | 2,320 | 56 | 1 | `GpuCooMatrixBuilder` (46) |
| `libCloAutoRetopology.dylib` | 2,176 | 1,963 | 212 | 1 | `QzipMesh` (157) |
| `libCloCV.dylib` | 1,570 | 381 | 144 | 1,045 | `__func<Marvelous` (60) |
| `libCloDeforMarvel.dylib` | 1,294 | 937 | 356 | 1 | `DeforMarvel` (336) |
| `libCloMath.dylib` | 754 | 675 | 78 | 1 | `MVVariant` (85) |
| `libCloSketch.dylib` | 530 | 344 | 185 | 1 | `Stroke` (101) |
| `libCloKnit_Core.dylib` | 434 | 273 | 160 | 1 | `knit` (61) |
| `libCloSimulateWRAPD.dylib` | 260 | 126 | 133 | 1 | `TriElements` (27) |
| `libCloSimulateEBP.dylib` | 179 | 68 | 110 | 1 | `Parafun` (26) |
| `libCloKnit_SwatchEditor.dylib` | 173 | 96 | 76 | 1 | `KnitEditorController` (24) |
| `libCloSimulatePhysics.dylib` | 132 | 81 | 50 | 1 | `WindProperty` (62) |
| `libCLOAPIInterface.dylib` | 121 | 25 | 3 | 93 | `APICommand` (24) |
| `libCloSimulateUtils.dylib` | 117 | 30 | 86 | 1 | `CriticalSection` (13) |
| `libCloSimulateDataStructures.dylib` | 97 | 72 | 24 | 1 | `KDTree` (23) |
| `libCloSimulateRootParity.dylib` | 74 | 32 | 41 | 1 | `RootParityCollisionTest` (12) |
| `libCloCore.dylib` | 22 | 5 | 16 | 1 | `clo` (5) |
| `libCloSimulateCommon.dylib` | 21 | 2 | 18 | 1 | `SimInterface` (2) |
| `libCloSimulateSolvers.dylib` | 19 | 8 | 10 | 1 | `Marvelous` (7) |
| `libCloSimulateCuda.dylib` | 5 | 5 | 0 | 0 | `Marvelous` (1) |
| **All 21 libraries** | **90,899** | **82,778** | **6,965** | **1,156** | — |

"Debug" counts macOS `-` stab records; `defined + undefined + debug == nm
entries`, and debug records are excluded from the rankings. "Top engine class"
is the most frequent owner token among *defined* symbols
(`Marvelous::Fabric::FabricItem::SetColor` counts under `FabricItem`), so it is
a count of symbol-table occurrences, not of methods, code size or importance.
The bundle-wide namespace ranking and the top 20 engine classes are in
[`libraries.json`](../data/clo3d/libraries.json); the engine classes are also
repeated in [`surface.json`](../data/clo3d/surface.json). The action and screen
registers below still come from the **main executable only**.

## B. Command surface by topic, against ClothCraft

CLO counts are **Observed**. The ClothCraft column is **my mapping** of those
topics onto this repository's code and its own documented limits — it is a
judgement, and the evidence column names where to check it.

| Topic | CLO | 2D / 3D / menu | ClothCraft status | Where to look |
|---|---|---|---|---|
| pattern_geometry | 184 | 105 / 70 / 9 | **Partial** | `garment/polygon.rs`, `garment/curves.rs`, `authoring.rs`: rectangle and pen, Bézier boundaries, constrained Delaunay, anchor/handle editing. No holes, no dimensions, no multi-selection. |
| other | 152 | 56 / 65 / 31 | **Mostly absent** | A genuine mixed bucket (z-order, UV editor, internal handlers). Not a topic to plan against; refine the classifier first. |
| sewing | 96 | 72 / 23 / 1 | **Partial** | `garment/seam.rs`, `atelier/edges_3d.rs`, `seams_ui.rs`: whole-edge sewing from either view, direction, easing, delete, undo. No partial edges, no 1:N or M:N, no notch correspondence. |
| interchange | 79 | 1 / 0 / 78 | **Partial** | `garment/export.rs` (per-panel OBJ, SVG, fibre frames), `avatar.rs` (OBJ import). No DXF-AAMA/ASTM, GLB, USD, Alembic, FBX, COLLADA. |
| editing | 73 | 34 / 19 / 20 | **Partial** | Gizmos, numeric transforms, undo/redo, duplicate/delete, magnet snapping. No mirror copies, no topology align. |
| avatar | 46 | 1 / 22 / 23 | **Partial** | `avatar.rs`: one static MakeHuman mannequin, BVH collider, signed distance. No avatar editor, no measurer, no AI avatar. |
| measurement | 43 | 7 / 34 / 2 | **Absent** | No POM, no measurement table, no size fitting. |
| trims | 43 | 0 / 22 / 21 | **Absent** | No zipper, button, topstitch or accessory model. |
| arrangement | 42 | 8 / 19 / 15 | **Partial** | `fitting.rs` + gizmo: rigid placement of panels around the mannequin. No body arrangement points, no automatic arrangement, no preflight intersection feedback. |
| simulation | 41 | 7 / 29 / 5 | **Implemented (core), Partial (breadth)** | `solver.rs` Newton/PCG, `blocksolve/` assembled nodal operator, `vbd.rs`, `ogc/`, `gpu_vbd.rs`. No wind, no layers, no friction, no pinching. |
| view | 39 | 11 / 14 / 14 | **Partial** | Camera orbit/pan/zoom, wireframe, strain and contact overlays. No multi-view, no layout save. |
| render | 36 | 8 / 13 / 15 | **Partial** | `render.rs` wgpu viewport with interpolated normals. No offline renderer, no lights, no shadows, no textures. |
| materials | 33 | 16 / 6 / 11 | **Partial** | `fabrics.rs`: nine estimated presets, per-panel grain, membrane/bending parameters. No texture maps, no PBR, no colourways. |
| grading | 26 | 24 / 1 / 1 | **Absent** | No grade rules, no size tables. |
| internal_lines | 26 | 22 / 4 / 0 | **Absent** | No internal lines, offsets or holes. |
| darts_pleats_folds | 21 | 19 / 2 / 0 | **Absent** | No darts, pleats or fold placement. |
| animation | 20 | 0 / 6 / 14 | **Absent** | No animation, cache or motion model. |
| notches_marks | 15 | 15 / 0 / 0 | **Absent** | No notches, annotations or instruction lines. |
| project | 13 | 0 / 1 / 12 | **Partial** | JSON template/project save and load through the CLI. No file picker, no autosave/recovery, no relative asset relinking. |
| automation | 7 | 0 / 0 / 7 | **Absent** | No scripting surface. CLO ships a headless Python controller. |
| print | 3 | 1 / 0 / 2 | **Absent** | SVG preview only; no scale-accurate printing. |

**Derived reading.** CLO's surface is weighted toward *authoring and product
breadth* — pattern geometry, sewing, grading, internal lines, darts, trims and
measurement dominate — not toward simulation. ClothCraft's strength is the
inverse: a verified solver with a narrow authoring surface. The largest
uncontested gaps by count are grading (26), internal lines (26), darts/pleats
(21), animation (20) and notches (15) — all small enough to be tractable, and all
currently blocked on the same missing thing: a richer document model with stable
semantic identities (`ROADMAP.md` DRAW-02/03/04, SEW-02).

## C. Screen register (**Observed**)

225 classes. The largest by recovered member count:

| Class | Members | Reading |
|---|---|---|
| `View2DActionSlots` / `View3DActionSlots` | 407 / 350 | The per-viewport command surfaces. |
| `DialogCloAvatarEditor` | 351 | Avatar editing is comparable in size to an entire viewport's commands. |
| `DialogPreference` | 256 | The preferences dialog alone is larger than ClothCraft's whole UI layer. |
| `DockWidgetController` | 222 | Docking is a first-class subsystem, not a fixed layout. |
| `DialogController` | 168 | Dialog lifecycle is centralised. |
| `DialogExportOBJ` / `DialogImportOBJ` | 144 / 129 | Import and export are separate, heavy screens — evidence that the interchange contract really is input/output-asymmetric. |
| `DialogMultiview` | 129 | Multi-view layouts exist as a feature. |
| `DialogColorwayList` | 121 | Colourway is a first-class data concept. |
| `DialogPatternDrafter` / `DialogAIParametricPattern` | 116 / 81 | Parametric and AI pattern drafting. |
| `DialogAIAvatarStudio` | 106 | AI avatar generation. |
| `DialogRegisterZipperObjectManager` / `DialogRegisterZipperObject` | 101 / 92 | Zippers are modelled objects with their own registration UI. |

Full list: [`screens.json`](../data/clo3d/screens.json).

## D. Shipped preset libraries (**Observed**)

1,579 files in 22 categories under `Contents/Resources/Preset/`. The extension
mix per category is itself evidence about what CLO models as data:

| Category | Files | Formats present |
|---|---|---|
| Avatar | 877 | `jpg` 536, `txt` 96, `pos` 39 |
| FabricCreator | 224 | `png` 191, `json` 15, `obj` 8 |
| DefaultImage | 107 | `pfm` 54, `png` 46, `svg` 5 |
| Skinsuit | 82 | `top` 79, `tss` 2, `zpac` 1 |
| Sewing | 56 | `png` 22, `jpg` 16, `stp` 6 |
| Knitwear | 52 | `json` 21, `npy` 20, `txt` 10 |
| Render | 47 | `png` 21, `vlp` 20, `hdr` 2 |
| Modular | 35 | `png` 17, `zmdr` 17 |
| FabricScanner | 30 | `svg` 25, `png` 4 |
| Parametric_Pattern | 17 | `eom` 9, `json` 8 |
| Gizmo | 13 | `png` 13 |
| Zipper | 13 | `png` 5, `zst` 4, `zsl` 1 |
| Bond | 6 | `zfab` 6 |
| Palette | 6 | `cco` 6 |
| Simulation_Property | 3 | `smp` 3 |
| Button | 2 | `btn` 1, `bth` 1 |
| ColorProfile | 2 | `txt` 1, `icc` 1 |
| PrintType | 2 | `png` 2 |
| Project | 2 | `zprj` 1, `json` 1 |
| Bond_Defual, Camera, Fold3D | 1 each | `zfab`, `obj`, `zfab` |

**Licensing boundary — read this before planning.** These are proprietary asset
libraries: fabric definitions, avatar meshes, render presets, palettes. Their
*count*, *file names* and *format extensions* are evidence about what CLO ships.
Their **contents must not be copied into ClothCraft**, and the register exists so
that ClothCraft can implement equivalent *mechanics* with its own, measured data.
The same applies to the 1,150 SVG icons and to any UI artwork.

## E. Formats and extensions (**Observed**)

71 distinct extensions under `Contents/` (frameworks excluded). By file count:
`png` 702, `jpg` 569, `zmod` 301, `txt` 143, `json` 110, `avs` 109, `zfab` 79,
`top` 79, `pfm` 54, `btn` 46, `pos` 39, `svg` 31, `cmat` 27, `dylib` 26,
`zpu` 22, `slist` 21, `npy` 20, `vlp` 20, `map` 19, `eom` 17, `zmdr` 17,
`obj` 16.

CLO-specific containers visible here include `.zfab` (fabric), `.avs` (avatar
shape), `.zmod`/`.zmdr` (modular design), `.zst`/`.zsl`/`.zpu` (zipper), `.smp`
(simulation property), `.cco` (palette), `.stp` (sewing), `.eom` (parametric
pattern), `.vlp` (render), `.zprj` (project). ClothCraft parses **none** of
them, and should not: they are proprietary asset formats, a different question
from reading a *user's own* project files.

## F. UI resources (**Observed**)

`Resources_Light.rcc` yields 1,393 names: 1,150 SVG, 205 PNG, 6 XML, 5 Python,
3 ICO, 3 ICNS. Fifty-eight of them match tool/icon naming and are listed in
[`tool_icons.json`](../data/clo3d/tool_icons.json). The five embedded Python
files are consistent with the scripting surface found in the symbol table
(`HeadlessApiController::ExecutePython`).

## G. How to use this to prioritise

1. **Filter by topic, not by total.** 1,038 commands overstate the work: many are
   preferences, internal handlers and duplicate entry points.
2. **Take the small high-count gaps first.** Grading, internal lines, darts and
   notches total 88 commands and none of them needs a new solver.
3. **Notice what the counts do not show.** CLO's `DialogPreference` (256) and
   `DockWidgetController` (222) together outweigh its whole 2D geometry surface
   in UI plumbing. Product surface, not algorithms, is where a clone spends its
   budget — the same conclusion the earlier architecture pass reached
   independently.
4. **Cross-check before building.** A name is a lead, not a specification. Any
   topic worth implementing should be decompiled and read before it is designed.

## H. Limits of this register

- Topic assignment is a heuristic; **152 actions (15%) remain unclassified**
  under `other`, and the classifier file is the place to fix that.
- Resource names are reported in name-table order with **directory nesting not
  reconstructed** — the tree section's child-offset encoding was not decoded.
  Treat them as an inventory, not a path listing.
- Preset *contents* were not parsed, only counted, named and typed by extension.
- The **action and screen registers cover the main executable only**. The 21
  `libClo*` libraries (`libCLOAPIInterface` included) are now symbol-scanned —
  90,899 `nm` entries, 82,778 defined, 6,965 undefined and 1,156 debug/stab
  records across their `arm64` slices — and their engine-side namespaces and
  engine classes are in [`libraries.json`](../data/clo3d/libraries.json). A
  user-facing command that lives only inside a library would not appear in
  `ACTIONS.md`.
- **`nm` reads the symbol table, not the code.** Code stripped of symbols,
  purely inlined functions and unnamed local labels are invisible, and the
  counts include symbols that are dead or unreferenced in this build. An
  undefined entry proves an import exists, not where or whether it is called.
- The library records are **counts and names, not behaviour**: no signature,
  parameter, default, cost or call graph is recovered, and this scan does not
  decompile any library. Cross-library ownership (which code calls which) is not
  established, so an engine class appearing in two libraries is counted twice.
- Demangling is `c++filt`; symbols it does not recognise stay as raw names and
  are counted in the totals, but C symbols and Objective-C runtime names such as
  `_OBJC_CLASS_$_FinderFilterHandler` carry no `::` scope and therefore do not
  contribute to the namespace or class rankings.
- No claim is made that every recovered name is user-visible.
