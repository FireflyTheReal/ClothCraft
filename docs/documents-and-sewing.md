# Documents and sewing

Implementation pass: 16 October 2026. A working alpha slice of SAVE-01, SEW-01 and SEW-02; the complete production roadmap remains open.

## Documents

File → New project starts one unsewn rectangle with the current avatar. Empty simulation meshes are unsupported. Open, Save and Save As use native file dialogs. Ctrl+O opens, Ctrl+S saves, Ctrl+Shift+S chooses another filename. An asterisk in the title marks unsaved edits or simulation changes. Opening a project clears the previous undo history.

Save stores the pattern model, materials, placements, sewing and the currently displayed accepted simulation state, plus VBD/contact settings. The CLI reuses saved settings unless `--dt` or `--iterations` explicitly overrides them. Placement previews are committed first; unfinished drawing/sewing drafts are not document changes. Export separately writes OBJ, preview SVG and research artifacts to the session output folder.

The JSON references a content-addressed OBJ in `<project filename>.assets/`. Move or copy **both** together. Save As creates its own adjacent asset directory. Open offers an OBJ picker if the avatar is missing. An invalid project or cancelled relink leaves the current garment intact.

Writes synchronize a temporary file in the destination directory before replacing the destination through `tempfile::NamedTempFile::persist`. Avatar assets are written first and are never overwritten with different content. A failed save preserves the previously saved document. This is not a guarantee against every filesystem or hardware failure. Source references: [tempfile persistence](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist), [native file dialogs](https://docs.rs/rfd/latest/rfd/struct.FileDialog.html).

Save / Discard / Cancel appears before opening, creating or closing with unsaved changes. Every 60 seconds, a dirty session writes a separate `recovery-<process id>.json` in its output folder. On startup the latest snapshot can be opened; File → Recover snapshot can choose another. Recovery opens as untitled until explicitly saved. The current session removes its recovery JSON after a successful save or confirmed close; asset directories may remain and are not automatically garbage-collected. Recovery is local to the output folder, not a global project browser.

## Partial sewing

1. Select Sew, then select A and B in either view. Full edges are the default.
2. Adjust From, To or Length in millimetres. Alternatively, enable **Pick partial range** and click two endpoints on the same boundary; repeat for the other side. Click order sets that segment's direction.
3. Drag the pink endpoint handles in either view. Labels identify side, segment and direction; the thick highlighted portion is the actual sewing interval.
4. Compare lengths. Reverse a segment or the entire B side if needed. Allow easing explicitly for unequal lengths, then Commit.

Existing seams have Edit and Delete actions in Properties → Piece. Edit is a draft until Apply; Escape or Cancel abandons it. A single-edge pair can still be committed with Simulate. Multi-edge drafts require explicit Commit or Cancel first.

Ranges use normalized **rest arclength**, not Bézier parameters or draped lengths. Two disjoint ranges on one boundary may belong to different seams. Overlapping ownership is rejected transactionally, including overlap within one chain. Remeshing retains semantic panel/edge references. An endpoint between mesh vertices uses interpolation; the surrounding mesh element still responds mechanically.

## Multi-edge sewing

Enable **Multiple edges per side**. Choose Add to A and click edges in order, then Add to B for the opposite chain. Enter switches side. Each side can contain one or many full or partial edges, including internal paths. Up changes order; Reverse changes one segment's orientation; × removes it from the draft. The entire B side can be reversed after assembly.

The compiler maps cumulative rest arclength across both sides. One 600 mm edge can match two 300 mm edges without duplicating the long edge's stiffness. A chain joint remains a discontinuity between separate geometric boundaries: no bridge is inserted across space. Integration uses the union of mesh breakpoints, chain joints and notch correspondences. Gap probes include both one-sided limits at joints. One relationship has one seam ID, undo entry and delete operation.

Imported notches are preserved when editing ranges/chains and apply over the entire normalized side. A dedicated notch editor and automatic migration are not implemented. User ordering is authoritative; there is no automatic sorting or inferred connectivity. Different panels may be chained before their endpoints meet in 3D. Easing is a length correspondence, not a calibrated gathering model.

## Implementation

- `document.rs`: atomic replacement and portable avatars, independent of UI.
- `fitting.rs`: v1 loading, state validation and saving.
- `atelier/documents_ui.rs`: path, dirty state, dialogs and recovery.
- `garment/seam.rs`: optional `BoundaryRef.reversed` and `SeamSpec.a_tail` / `b_tail`. A/B remain the first segment; tails are ordered continuations. Defaults are omitted, preserving legacy single-edge JSON and physical signatures.
- `BoundaryCurve::concatenate`: cumulative-length paths compiled to existing seam energy terms; no new solver.
- `Command::SewChain`: create/replace one seam transactionally; `Command::Sew` now accepts partial ranges.
- `atelier/sewing_ui.rs`: shared draft, picking, range handles and controls.

Old files load without conversion. Older builds denying unknown fields reject files using the extensions. Clipboard copies include a seam only when **every** referenced panel is selected. Mirrors reverse segment ranges/directions coherently; deleting a referenced panel removes its whole seam. Topology operations unable to preserve a sewn boundary still reject the edit explicitly.

`tests/documents.rs` covers relocation with cached state and failed replacement. `tests/sewing_chains.rs` covers partial support, disjoint/overlapping ownership, 1:N/M:N integration, derivatives, serialization, remeshing, undo and clipboard references. Native acceptance is recorded separately after execution; test counts are not a claim of production reliability.

## Native acceptance

The 55-action native scenario passes on 16 October 2026: rectangle/Bezier creation, mixed 2D/3D sewing, range edits, M:N chain editing, deletion, undo/redo, transformations, fabric assignment, portable export/reload and actual motion of the added piece. [Report](document-sewing-validation.json). [Editable fixture](../examples/templates/partial-chain.json). The smoke uses real egui input but does not automate the OS file picker or force-kill recovery. Those remain manual acceptance cases; portable storage and failed-save behavior have CPU tests.

Three explicit GPU steps on the exported 946-vertex chain scene passed the f64 force comparison on NVIDIA RTX 3090 / Vulkan; maximum absolute gradient errors were 6.533e-5, 5.016e-5 and 4.296e-5 N. This validates force assembly for this fixture, not convergence, fit quality or a general performance claim.

Final checks: `scripts/check-cpu.ps1` passes 132 CPU tests, CPU Clippy and six template fixtures. Desktop checks pass 18 library, 39 UI and two shader tests; the optional layout-image exporter is ignored. All-target Clippy and formatting pass. GPU/window runs were explicit and remain outside the default suite. Logs are under `output/document-sewing-*-final.log`. The new fixture retains canonical Rust serialization; historical fixtures were not rewritten.
