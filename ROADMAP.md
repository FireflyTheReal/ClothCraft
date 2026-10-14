# ClothCraft roadmap

Build a useful garment authoring tool whose physics can evolve independently. Public Marvelous Designer manuals inform expected interactions; ClothCraft implements its own code, data model, UI and test fixtures. This is an independent implementation effort, not an affiliated product or a claim of formal clean-room certification.

Status: first public source snapshot, October 2026. **A roadmap item is not a supported feature.** Read the [source/behavior ledger](docs/manual-study.md) and [current authoring guide](docs/authoring.md).

## 1. Draw → sew → arrange → simulate

- [x] Unified metric 2D board, rectangle/pen creation with straight and cubic Bézier boundaries; constrained Delaunay meshing.
- [x] Linked 2D/3D panel/edge selection, duplicate/delete, whole-edge sewing from either view, direction and explicit easing.
- [x] World-axis translation/rotation gizmos, numeric transforms, transactional preview and undo/redo.
- [x] Save editable templates/projects and reopen through the CLI; preserve v1/v2 data compatibility.
- [x] Edit existing anchors and Bézier handles; pan/zoom, snapping and saved 2D layouts.
- [ ] Exact dimension entry, holes, internal lines, multi-selection and symmetry.
- [ ] Partial/free sewing, notch editing, 1:N / M:N relationships and clearer 3D direction preview.
- [ ] Arrangement points, body-aware placement and useful preflight intersection feedback.
- [ ] File picker, multi-document projects, portable asset bundles and recovery/autosave.
- [ ] Seam allowance, grain annotations, graded sizes and print layouts.

Acceptance: an unfamiliar user can draft a simple top, join its intended boundaries, place it outside the avatar, simulate, save and reproduce the same editable garment. New formats must not store transient mesh indices as sewing references.

## 2. Physics as a replaceable module

- [x] Authoring and material-space model independent of egui/wgpu; CPU-only build.
- [x] CPU Newton reference; CPU VBD; hybrid GPU VBD/OGC; real window timing and force checks.
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

Every adapter must declare supported entities, units, axes/handedness, topology policy, material mapping and losses. Reject or report unsupported data instead of silently flattening it. Put adapters around the canonical garment model, not inside a solver. Add round-trip tests and open-source or self-authored fixtures with provenance.

## 3. Public-manual study queue

Read and record small feature families: pattern construction/editing → sewing → arrangement → simulation interaction → fabric parameters → import/export → production outputs. Track observations, our independent specification, implementation status and acceptance cases in [manual-study.md](docs/manual-study.md). The full manual has not been exhaustively reviewed.
