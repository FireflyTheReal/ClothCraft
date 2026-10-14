# Unified pattern workspace

Launch `scripts/start-atelier.ps1`. All pieces share **one metric 2D board**, beside the 3D garment view. A panel or named boundary selected in either view identifies the same model object in the other view.

## Draft cloth

- **Nouveau** starts a blank drawing; the existing garment is replaced only after a valid contour is completed. Cancel the trace to keep it.
- **Plume**: click for a corner; click and drag to set a Bézier tangent. Continue around the piece, then click its first anchor or **Fermer le contour** / Enter. Backspace removes the last anchor; Escape cancels the trace. The optional magnet snaps to 10 mm.
- **Rectangle**: drag between opposite corners. Release creates a meshed panel in 2D and 3D.
- **Modifier**: select a panel, then drag an anchor to change its outline. Select a boundary and use **Ajouter poignées Bézier**; drag either blue handle independently. **Convertir en segment droit** removes the handles. Self-intersecting or otherwise invalid edits are rejected without changing the document.
- Drag the interior of a piece to rearrange it on the board. This changes only its 2D layout, never its physical rest shape or 3D placement. Middle/right drag pans the board; the wheel zooms around the pointer; **Tout cadrer** frames all pieces.

Curves are cubic Béziers with two controls per named edge. They are stored as curves, then subdivided to a 0.25 mm chord-error bound for the constrained Delaunay mesher. Sewing uses that sampled boundary's rest arclength, not the straight chord between its anchors. No holes, internal cuts, automatic smooth-handle coupling or multi-selection yet. Legacy v1 rectangular templates remain readable/simulatable; contour editing targets v2 polygons, including newly drawn rectangles.

## Sew in either view

Choose **Coudre** and click boundary A in 2D or 3D, then boundary B in either view. Edges highlight together across views; endpoint labels (`A0 ↔ B0`, `A1 ↔ B1`) and 3D connection lines preview correspondence. Check the length difference and **Inverser le sens B**, then **Créer la couture**. Different lengths require **Autoriser l'embu**.

3D picking follows compiled boundary chains, including curves and the current drape. It rejects hits hidden by the avatar or other cloth: orbit to reach the back. Highlight lines are overlays and may remain visible through geometry. A whole panel can be selected by clicking its visible surface in **Modifier** or **Placer 3D**.

Expand **Coutures existantes** to remove a seam before reusing a boundary. This editor handles whole named edges; partial/free sewing, interactive notches and M:N relationships remain planned. Editing one member of an equal-length seam can be rejected if it changes that length: remove/recreate the relation with explicit easing when appropriate.

## Place and simulate

**Placer 3D** returns to the initial flat arrangement. Use the colored arrows / rotation rings or numeric values; **Cadrer le panneau** finds an off-screen selection. Gizmo release commits, Escape discards; numeric fields use **Appliquer**. Right drag orbits, middle drag pans, wheel zooms the 3D camera. **Dupliquer / Supprimer** act on the selected panel; deletion removes its seams and retains at least one panel.

**Simuler / Space** switches to cloth-grab mode and starts the existing worker. In **Saisir**, left drag applies a barycentric force. Shape, seam and 3D placement edits reset the simulated garment and invalidate solver caches; a 2D layout edit preserves the physical state. Gizmo/contour previews do not remesh each frame. Neither a drawing nor a gizmo guarantees a collision-free initial arrangement; OGC can reject cloth placed inside the body.

**Annuler / Rétablir**, or Ctrl+Z / Ctrl+Shift+Z, retain up to 32 document edits. While drafting, Ctrl+Z removes an anchor. The inspector shows relevant tools; mesh density and simulation diagnostics are under **Résolution du tissu** / **Réglages avancés**. **Exemples** loads the T-shirt or dress; **Affichage** contains mesh, strain and seam overlays.

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

Optional `curves` and `layout_2d` fields extend existing templates; old v1/v2 files still load. `layout_2d` is excluded from physical cache signatures. JSON uses exact floating-point round trips so near-degenerate Delaunay decisions do not change on reload. Curve intersection validation applies to the sampled contour, not an exact algebraic intersection proof.

The explicit native smoke (GPU window, outside ordinary CPU CI) is:

```powershell
cargo run --release --locked -j 2 --bin cloth-fit -- --steps 0 --ui-authoring-smoke --output output/board-smoke
```

It uses real egui pointer events and writes `authoring.json` plus `ui.png`. CPU coverage is in `tests/authoring.rs` and `tests/pattern_board.rs`.

Measured outcomes and example captures: [board validation](board-validation.md). The reusable [curved-panels fixture](../examples/templates/curved-panels.json) is a saved result from the native drawing workflow.
