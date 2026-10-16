# Workspace UI pass — 15 October 2026

The workspace now gives most of the window to the garment and the unified pattern board. This is an independent interface pass, not a reproduction of proprietary artwork. All new controls and documentation are in English; localization remains future work.

## Controls

- **Left tool rail:** Edit, Pen, Rectangle, Sew, Internal line, Move 3D, Rotate 3D and Grab, followed by Undo/Redo. Hover for names and shortcuts. Right-click either canvas for the existing radial picker.
- **Resizable views:** drag the divider between 3D and 2D, or the divider before Properties. Double-click a divider to restore its default. **Expand** focuses a view; **Split** restores both with the previous ratio. **Workspace** can show/hide the inspector or reset the layout.
- **3D header:** frame the current garment/avatar bounds, open camera presets/projection, or use **gear ▾** for viewport display and overlays.
- **2D header:** frame all patterns and use **gear ▾** for snapping and board rotation. Shapes, selection/clipboard actions and New sheet are in the compact row below. Finish/Cancel appear during a trace; precise dimensions/points live in Properties.
- **Properties:** the piece browser stays above the **Piece / Fabric / Simulation** tabs. Piece contains seams, dimensions, point/boundary tools and placement. Fabric contains the existing shared material controls. Simulation contains resolution, solver parameters and diagnostics.

Preferences are saved in the session's `preferences.json`. Layout and display changes persist when pointer interaction ends, rather than writing on every drag frame. Old preference files receive defaults for the new fields. These settings are not garment-project fields and never change material coordinates, contact, mass, seam definitions or simulation state.

## Display implementation and limits

`render/appearance.rs` owns display values. `render/environment.rs` and its WGSL implement a fullscreen environment pass with a depth-tested ground plane. The renderer retains a single-sample texture for egui and screenshots; optional **4× MSAA** uses matching color/depth attachments and resolves into that texture. Unsupported adapters fall back to one sample and the 4× control is disabled. Changing sample count rebuilds the pipelines and their compatible bind groups; resizing recreates the attachments. MSAA costs graphics memory and bandwidth and is not a simulation optimization.

Background choices are Studio/Light/Dark presets or custom solid/gradient colors. Ground color, grid, shadow and shadow opacity update live. The grid uses screen derivatives to soften distant lines. **Soft ground shadow is an approximate elliptical contact-style preview beneath the combined model bounds**, not an occlusion-aware shadow map or ray-traced light. The ground is visual only, at the existing preview height (−5 mm with an avatar); it adds no physical floor. Environment colors follow the existing unorm viewport pipeline, not a calibrated color-management workflow.

The regular cloth view also skips strain evaluation when the strain overlay is off. This removes unnecessary CPU work; no FPS gain has been measured for this pass.

## Validation and follow-up

CPU-only checks cover old preference compatibility, sample-count fallback, splitter geometry/drag input, compact header bounds, camera behavior and WGSL parsing/validation including the 208-byte environment uniform layout. The optional `export_chrome_preview` test produces the real egui header/display components with placeholder canvases for layout review. It creates no wgpu instance, adapter or worker and is not a native 3D screenshot.

Validation completed on 15 October with two jobs/threads:

- `scripts/check-cpu.ps1`: 127 tests passed, CPU Clippy clean, five templates validated.
- `cargo test --locked -j 2 --lib --bin cloth-fit --test shader -- --test-threads=2`: 18 library, 39 desktop UI and two shader tests passed. The final UI-only rerun also passed; the optional preview test was run explicitly and visually inspected after CPU rasterization.
- `cargo clippy --locked --all-targets -j 2 -- -D warnings`: clean after the final source edits. `cargo fmt --all -- --check` and `git diff --check` also passed.
- Local logs: `output/ui-cpu-check.log`, `output/ui-desktop-tests.log`, `output/ui-desktop-tests-final.log`, `output/ui-desktop-clippy-final.log`. The layout-only image is `output/ui-layout-preview.png`; it intentionally contains placeholder canvases.

Native GPU validation is intentionally deferred while the owner's GPU is rendering in Unreal. Before calling this renderer verified, exercise resize and expand/restore, Off ↔ 4× MSAA, perspective/orthographic/top/bottom views, custom backgrounds, ground/grid/shadow toggles and screenshot resolve on a supported adapter. Also rerun the explicit authoring smoke (now opening the Fabric tab). No performance or jitter improvement is claimed by this UI pass. Docking arbitrary floating windows, lighting presets/HDRI, actual cast shadows, TAA, localization and saved camera/board poses remain unimplemented.

## Sources and implementation map

The supplied Marvelous Designer screenshot was visually inspected for its two large work areas and nearby tools. The public [3D Window](https://support.marvelousdesigner.com/hc/en-us/articles/47358233197593-3D-Window) and [3D Window Display Icons](https://support.marvelousdesigner.com/hc/en-us/articles/47358200025497-3D-Window-Display-Icons) articles were read for viewport-local controls and display categories. The linked online screenshot was found but not successfully rendered for visual inspection. No external interface assets were copied.

- `atelier/layout.rs`: persistent pane state, geometry and draggable dividers.
- `atelier/chrome.rs`: shared compact headers, appearance popup, inspector tabs, 3D canvas adapter.
- `atelier/board.rs`: unchanged editing transactions, compact 2D chrome.
- `atelier/settings.rs`: backward-compatible preference load/save.
- `render.rs`, `render/appearance.rs`, `render/environment.*`: display only, independent of physics.
- `atelier/ui_preview.rs`, `tests/shader.rs`: headless input/layout and shader validation.

## Native verification — 16 October

GPU use was authorized for this pass. The 55-action authoring scenario now passes, including the new partial/M:N sewing workflow and actual simulated motion. The native pass also caught and fixed rectangle drags losing canvas ownership when contextual controls appeared; the canvas now has a stable interaction ID.

The real 1600×980 window was captured with 4× MSAA, gradient, ground, grid and the approximate ground shadow. A second 1200×800 capture exercised the dark theme, MSAA Off, solid background and hidden ground/grid/shadow. Both were visually inspected; [the README image](screenshots/workspace.png) is the actual first capture, after 120 explicitly requested GPU steps on the bundled shirt. Interactive expand/restore and every camera/popup combination have not been rechecked natively. Local outputs: `output/document-workspace-preview/`, `output/document-workspace-off/`, `output/document-sewing-native-v4/`.
