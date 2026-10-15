# Unified pattern workspace

Launch `scripts/start-atelier.ps1`. All pieces share **one metric 2D board**, beside the 3D garment view. A panel or named boundary selected in either view identifies the same model object in the other view.

## Draft cloth

For numeric primitives, mirrors, booleans/holes and internal sewing curves with ruffles, see [drafting tools](drafting-tools.md). All use the same board, selections and undo history.

- **Nouveau** starts a blank drawing; the existing garment is replaced only after a valid contour is completed. Cancel the trace to keep it.
- **Plume**: click for a corner; click and drag to set a Bézier tangent. Continue around the piece, then click its first anchor or **Fermer le contour** / Enter. Backspace removes the last anchor; Escape cancels the trace. The optional magnet snaps to 10 mm.
- **Rectangle**: drag between opposite corners. Release creates a meshed panel in 2D and 3D.
- **Modifier**: select a panel, then drag an anchor to change its outline. Select a boundary and use **Ajouter poignées Bézier**; drag either blue handle independently. **Convertir en segment droit** removes the handles. Self-intersecting or otherwise invalid edits are rejected without changing the document.
- Drag the interior of a piece to rearrange it on the board. This changes only its 2D layout, never its physical rest shape or 3D placement. Middle/right drag pans the board; the wheel zooms around the pointer; **Tout cadrer** frames all pieces.
- In **Modifier**, drag the selected piece's gold circular handle to rotate it on the board. **Shift** snaps to 15° increments; Escape cancels the preview and Ctrl+Z undoes the edit. Rotation keeps the piece's center fixed and preserves material coordinates, grain, seams and the complete simulated state. Anchors and Bézier handles remain editable after rotation.

Curves are cubic Béziers with two controls per named edge. They are stored as curves, then subdivided to a 0.25 mm chord-error bound for the constrained Delaunay mesher. Sewing uses that sampled boundary's rest arclength, not the straight chord between its anchors. Boolean cutouts create holes, and internal sewing lines retain material on both sides. Internal cuts, automatic smooth-handle coupling for outer contours and multi-selection remain planned. Legacy v1 rectangular templates remain readable/simulatable; contour editing targets v2 polygons, including newly drawn rectangles.

## Sew in either view

Choose **Coudre** and click boundary A in 2D or 3D, then boundary B in either view. Edges highlight together across views; endpoint labels (`A0 ↔ B0`, `A1 ↔ B1`) and 3D connection lines preview correspondence. Check the length difference and **Inverser le sens B**, then **Valider la couture**. Different lengths require **Autoriser l'embu**. The highlighted pair is explicitly labelled as a preview until committed. **Coudre et simuler / Space** commits the pending pair before starting; invalid sewing leaves simulation paused and displays the reason in the inspector.

Committed seams have a stable color in both views and the seam list. Sparse dashed links and small endpoint ticks connect corresponding positions. Selecting or hovering either edge emphasizes its pair; pending links are pink and labelled **Aperçu A ↔ B**. **Affichage → Liens de couture 2D + 3D** hides the background links while retaining the active pair. The display follows the compiler's trimmed, reversed, notch-aware rest-arclength map; no vertex indices are persisted. Lines are visual overlays, not threads or additional constraints, and may be visible through geometry.

For an apron added to an existing shirt: draw the rectangle, place it outside the body, select its upper edge and the shirt's free hem, check correspondence, then **Coudre et simuler**. For example, a 660 mm edge joined to a 560 mm hem needs explicit easing or a pattern adjustment. A newly drawn piece participates in physics even before it is sewn; without an attachment it falls.

3D picking follows compiled boundary chains, including curves and the current drape. It rejects hits hidden by the avatar or other cloth: orbit to reach the back. Highlight lines are overlays and may remain visible through geometry. A whole panel can be selected by clicking its visible surface in **Modifier**, **Déplacer 3D** or **Tourner 3D**.

**Coutures existantes (N)** is always available at the top of the inspector: select a named seam or press its **Supprimer** button. In **Modifier** or **Coudre**, click a sewn boundary in either view: both sides highlight and **Supprimer la couture** appears. **Suppr** removes the seam if exactly one is associated with the selected edge and no text field owns keyboard input. **Ctrl+Z** restores it. Removal preserves the cloth and drape. This editor handles whole named edges; partial/free sewing, interactive notches and M:N relationships remain planned. Editing one member of an equal-length seam can be rejected if it changes that length: remove/recreate the relation with explicit easing when appropriate.

## Place and simulate

**Déplacer 3D (G)** and **Tourner 3D (R)** directly select the arrangement gizmo. Drag the colored arrows or X/Y/Z rings; Shift snaps rotations to 15° increments. Rings remain draggable when viewed edge-on using a screen tangent. Rings and numeric rotations use the panel's bounding-box center as their pivot; **Cadrer le panneau** finds an off-screen selection. Shortcuts do not act while editing a text field.

These tools edit the selected piece's **flat arrangement** and preserve the other pieces' drape. Moving or rotating a draped piece returns that piece to its flat shape; it is not a rigid transformation of the drape. Gizmo release commits, Escape restores the state before preview; numeric fields use **Appliquer**. Right drag orbits, middle drag pans, wheel zooms the 3D camera. **Dupliquer / Supprimer** act on the selected panel; deletion removes its seams and retains at least one panel.

**Simuler / Space** switches to cloth-grab mode and starts the existing worker. In **Saisir**, left drag applies a barycentric force. Physical edits invalidate solver caches but retain positions and velocities of compatible panels, matched by ID and identical local rest mesh, arrangement and pins. Adding a piece or changing seams/material therefore preserves the existing drape; newly drawn, moved or remeshed pieces start at their arrangement. Density edits rebuild masses without resetting geometry or velocity. A 2D layout edit preserves the whole physical state. **Simulation → Rétablir l’arrangement à plat** deliberately restores the full arrangement. Gizmo/contour previews do not remesh each frame. Neither a drawing nor a gizmo guarantees a collision-free initial arrangement; OGC can reject cloth placed inside the body.

## Choose a fabric

Open **Tissu · poids et stretch** in the selected panel's inspector. Nine indicative presets cover cotton, jersey, wool, denim, leather, silk, linen, nylon and stretch knit. Adjust g/m², thickness, extension at the displayed reference load, shear and bending, then **Appliquer le tissu**. Use the checkbox to affect all panels. The inspector shows actual panel and garment masses in grams. [Fabric guide and sources](fabrics.md) distinguish measured mass/thickness from estimated mechanics. These controls change physical properties, not visual textures.

**Annuler / Rétablir**, or Ctrl+Z / Ctrl+Shift+Z, retain up to 32 document edits. While drafting, Ctrl+Z removes an anchor. The inspector shows relevant tools; mesh density and simulation diagnostics are under **Résolution du tissu** / **Réglages avancés**. **Exemples** loads the T-shirt or dress; **Affichage** contains mesh, strain and seam overlays.

The **Pièces** browser also selects a panel in both views. **Simulation** contains single-step and full-arrangement reset; **Vue générale** reframes the avatar. **Fichiers & formats** lists only actual interchange capabilities and the export directory. Tooltips explain gestures; drawing, sewing, placement and grabbing are grouped in the toolbar. See [workspace validation](workspace-polish.md).

## Save and reproduce

**Exporter** writes template JSON, fitting-project JSON, OBJ, preview SVG and fiber guides in the launch output directory, replacing its fixed filenames. Bézier controls and 2D layout survive JSON round trips; SVG outlines and sewing previews follow the sampled curves. SVG remains a preview, not a calibrated sewing print.

Reopen with `scripts/start-atelier.ps1 -ProjectPath output/atelier/fitting-project.json`. The fitting project currently stores a resolved avatar path; relink it when moving to another machine. A file picker, portable bundle and autosave remain planned.

## Implementation map

| Module | Responsibility |
|---|---|
| `src/authoring.rs` | Atomic commands, physical-vs-layout change classification, history, local-coordinate normalization |
| `src/garment/curves.rs` | Cubic evaluation, bounded recursive subdivision, sampled arclength |
| `src/garment/polygon.rs` | Contour validation, triangulation, reassembly of named curved boundary chains |
| `src/atelier/board.rs` | Unified pan/zoom canvas, drafting, anchor/handle/layout previews, shared selection |
| `src/atelier/edges_3d.rs` | Screen-space edge picking with visibility checks and correspondence overlays |
| `src/atelier/authoring_ui.rs`, `gizmo.rs` | Inspector, history, mode controls, rigid placement |

Optional `curves`, `holes`, `lines`, `layout_2d` and `layout_rotation_degrees` fields extend existing templates; old v1/v2 files still load. Empty geometry extensions are omitted when serialized. Both layout fields are excluded from physical cache signatures and physical-change comparisons. The angle defaults to zero and zero is omitted when serialized, preserving historical cache signatures. JSON uses exact floating-point round trips so near-degenerate Delaunay decisions do not change on reload. Curve intersection validation applies to the sampled contour, not an exact algebraic intersection proof.

The explicit native smoke (GPU window, outside ordinary CPU CI) is:

```powershell
cargo run --release --locked -j 2 --bin cloth-fit -- --steps 0 --ui-authoring-smoke --output output/board-smoke
```

It uses 41 real egui pointer actions and writes `authoring.json` plus `ui.png`, including 2D rotation preserving physics, all three 3D rotation rings, pending-seam commitment on Play, seam deletion/restoration preserving the drape, a denim material assignment, export and actual new-panel motion. The fixture separates pieces before turning them: arrangement tools do not repair or prevent initial intersections. CPU coverage is in `tests/authoring.rs`, `tests/pattern_board.rs`, `tests/editing_state.rs` and `tests/fabrics.rs`. GPU free-fall and supported-cloth regressions are explicitly ignored by default; see [adaptive initialization](adaptive-initialization.md).

Rotation validation, 14 October 2026: all 78 CPU tests and the 41-action native scenario pass ([captured report](rotation-validation.json)). The existing three-panel saved fitting project also reloads with its state and historical signature. The first XYZ fixture intersected its neighbour and was rejected by OGC; the final fixture adds an explicit translation before rotation. This is an interaction/state-preservation check, not a convergence or performance benchmark.

During simulation the status bar displays maximum seam gap and positive axial extension (95% of rest area and maximum). Large gap means assembly is incomplete; persistent excessive extension may indicate incompatible sewing, material choice or insufficient convergence. Advanced settings allow up to 128 iterations. The default is now 16; adaptive initialization and the larger solve budget address artificial gravity stretch without changing mass or gravity.

Measured outcomes and example captures: [board validation](board-validation.md). The reusable [curved-panels fixture](../examples/templates/curved-panels.json) is a saved result from the native drawing workflow.
