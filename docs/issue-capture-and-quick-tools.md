# Issue capture and cursor tools

## Mark a simulation problem

Open the saved garment, reproduce the problem, then choose **Mark issue** on the left rail or press **D**. This pauses on the displayed accepted state. Paint the problem faces in the 3D viewport; **Shift** erases. The brush samples visible cloth, so it does not paint through the avatar or the front cloth layer. Orbit to reach the back. Marks are red, depth-tested in 3D, and also appear on the material-space pattern.

Use the brush size slider, add an optional description, then **Export report**. The application creates a unique folder under `output/diagnostics/` and copies its full path to the clipboard. **Copy path** retrieves it again. The folder contains:

- `project.clothcraft.json` and its adjacent avatar asset directory: editable geometry, materials, seams, the accepted positions/velocities and simulation settings.
- `report.json`: the description, marked faces identified by panel and three material-space coordinates, per-face stretch, current/rest area ratio and minimum altitude, vertex velocities/masses/pins, fresh contact bounds and nearby contact counts; global seam gaps, the highest-extension, most compressed and most restricted faces. The count below one percent of rest area is diagnostic only; it does not affect simulation or certify intersections.
- `README.txt`: replay instructions and scope.

Tell the person or agent debugging the scene where the folder is, or share the whole folder. **Export does not upload anything, send a message or automatically notify an AI.** A report should be inspected before sharing because it includes the garment and avatar.

Marks are a transient diagnostic overlay, not physical pins or editable mesh indices. Loading another document or making a physical edit clears them; export before changing geometry. Layout-only edits retain them. They are not included in ordinary Save. Entering the marking tool pauses; **Done** returns to Edit and **Space** resumes simulation. A large brush samples the visible surface rather than selecting every triangle intersecting a perfect projected disk.

Uncommitted drawing and sewing previews are not part of the exported document. The capture controls show a reminder when a draft is present.

The contact probe runs once during export, on CPU, using the existing solver's topology and seam exclusions. It recomputes conservative bounds with a query radius of twice the offset radius. Those are not the live worker's cached bounds. A small bound is evidence of a restricted neighborhood, not proof of an incorrect collision or a stalled vertex. A single saved frame cannot establish jitter. The solver's prediction/contact caches are not serialized, so replay begins from the same accepted state but need not follow a bit-identical trajectory. No automatic repair or physical correction is applied.

The same probe can inspect a saved scene without opening a window or advancing physics:

```powershell
cargo run --locked --no-default-features -j 2 --example issue_probe -- path/to/garment.clothcraft.json output/diagnostics
```

## Commands beside the canvas

**Q** opens Quick actions at the cursor. **Quick · Q** above the pattern board opens the same palette. Escape, Q again or a click outside closes it; closing clicks are blocked from the canvases. The existing right-click radial and right-drag navigation remain available.

- **Pen / Rectangle / Ellipse / Arc:** choose a drawing tool and draw on the 2D board. Drag the bounding corners for shapes. Hold Shift for equal width/height (circle or square). Arc creates a closed sector; its sweep is set in Quick actions. Numeric dimensions remain in Shapes and Properties.
- **Flip X / Flip Y:** reflect the active piece in its local pattern coordinates.
- **Copy −X / +X / −Y / +Y:** create and select an independent mirrored copy beside it, with one undoable command. These are not linked symmetry instances.
- **Union / A − B / Intersect:** Shift-select exactly two pieces or use the existing box selection. The active piece is A; both IDs are displayed. Swap A/B changes subtraction order. Geometry uses their overlap on the 2D board. Keep B retains the second operand. Existing validation rejects sewn, pinned, internally constrained or targeted operands; failures leave the garment unchanged and display the error in the palette.

The extra shape and boolean glyphs extend ClothCraft's original vector icon set. There are no external icon downloads or additional asset licenses.

## Sewing without leaving the views

The Sew tool displays lengths and **Flip / Ease / Sew** controls immediately above both canvases. The controls share the same draft as the inspector.

- **F**, or **Shift + wheel** over a canvas, reverses the entire B side. The highlighted endpoint correspondence updates in both views. Ordinary wheel movement still zooms.
- **E** toggles explicit easing for unequal rest lengths. Easing changes length correspondence; it does not make the physical material more elastic.
- **Enter** commits a single pair. In chain mode, Enter still switches the input side; **Ctrl+Enter** commits the chain.

Both sides must exist before committing. Mismatched lengths without easing are still rejected. Partial range editing, individual segment reversals and ordered chains retain their existing inspector controls. No automatic matching or inferred chain ordering is introduced.

## Implementation and validation

`issue_report.rs` and `examples/issue_probe.rs` are CPU-only. `atelier/issue_ui.rs` owns transient painting; `picking::visible_cloth_face` supplies avatar-aware nearest hits. The renderer colors marked faces within its ordinary depth pass. `atelier/quick_actions.rs` adapts the existing authoring transactions; `sewing_ui.rs` owns the shared viewport strip and shortcuts.

`tests/issue_report.rs` checks material-space references, portable replay, preservation of positions/velocities/settings, stale-face rejection and avatar occlusion. The optional native quick-tools scenario uses real pointer events:

```powershell
$env:CLOTHCRAFT_AUTHORING_SCENARIO = 'quick'
cargo run --locked -j 2 --bin cloth-fit -- --steps 0 --ui-authoring-smoke --output output/quick-tools-smoke
Remove-Item Env:CLOTHCRAFT_AUTHORING_SCENARIO
```

This explicitly uses a GPU window. The regular authoring scenario is unchanged when the variable is absent.

### Verified on 16 October 2026

- `scripts/check-cpu.ps1`: 134 tests passed, CPU Clippy passed, all six example templates validated. No GPU tests are part of this command.
- Desktop-enabled Clippy passed with `--all-targets -- -D warnings`; 40 editor unit tests and both issue-report integration tests passed (one optional preview test ignored). Builds used `CARGO_INCREMENTAL=0` after a compiler incremental-cache read failure.
- Native quick-tools scenario: 23/23 pointer actions and all nine outcome checks passed (`output/quick-tools-native-v4/authoring.json`). This covers union and undo, a mirrored copy, ellipse/sector drawing, sewing direction/easing controls, visible face painting and a report reopening with identical saved state.
- Native authoring regression: 55/55 pointer actions passed (`output/quick-tools-regression-v1/authoring.json`), including partial/multi-edge sewing and motion of a newly drawn panel.
- The optimized build opened the supplied saved garment in a native GPU smoke test with zero simulation steps; its final screenshot was inspected (`output/garment-ui-final/ui.png`). The original saved file was preserved.

The private garment supplied for debugging has 6,486 vertices and 12,284 triangles. A CPU probe of its saved frame found seven vertices with fresh displacement bounds below 10 micrometres and 22.42% positive extension at P95. A separate three-step GPU continuation retained a maximum seam gap of 52.26 mm on `seam_2`. These are evidence of unresolved deformation/contact behavior, not a confirmed cause of jitter. This change adds capture and editing tools; it does not alter the physics. The user's scene and generated reports remain local and are not repository fixtures.
