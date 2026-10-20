# Panel selection, measurement and linked mirrors

19 October 2026. CPU authoring changes; solver dynamics are unchanged.

## Internal line

Choose **Internal line**, then click a boundary in 2D. Its owner becomes the active
panel and appears above the canvas. This click selects the target; it does not
place an endpoint on the contour, because internal paths must currently remain
strictly inside material. Click inside to start, drag for Bézier tangents and press
Enter or **Finish line**. Starting directly inside a panel also chooses that panel.
While a path is in progress, another piece cannot silently take ownership.
Escape cancels. Boundary selection in either view is also used when activating
the tool. Joined, crossing or boundary-terminating internal paths remain unsupported.

## Measurements and properties

**Measure (M)** is available in the vertical toolbar and **Quick (Q)**. Click two
points on the 2D board for distance, signed X/Y differences and angle. A third
click begins another measurement; Escape clears it. Existing snapping applies;
Ctrl suspends snapping. It is a read-only board ruler, not a 3D geodesic tape or a
saved dimensional constraint. Boundary arclength and target-length controls remain
separate. Panel selection brings the Piece inspector forward; its material button
opens Fabric controls. Rest area and triangle count are displayed beside dimensions.

## Linked mirrored pieces

In Piece properties or **Quick (Q)**, use **Linked copy X/Y**. This creates a
separate mirrored piece and a bidirectional edit relation. Edits to contours,
curves, internal lines, material pins and grain propagate on commit/mouse release
from either piece. Both updates are one history transaction. The existing partner
keeps its identity, fabric, board position and 3D placement. There is no live
partner preview during the drag and no symmetry within a single panel yet.

**Unlink mirror** removes only the relation, leaving geometry and drape intact.
Independent mirror/copy commands remain available. Each panel has at most one
partner: chains/cycles are rejected. Split/delete-point, booleans and in-place flips
require unlinking first. Invalid geometry or incompatible sewn lengths reject the
entire edit, including propagation. No seam is silently deleted or reversed.
Deleting a panel removes its relationship. Copy/paste preserves a relation only
when both partners are copied, assigning fresh panel IDs.

The optional `GarmentTemplate.mirror_links` field stores `{a,b,axis}` using panel
IDs (`axis` is `x` or `y`). Empty lists are omitted, preserving historical files.
Relations are excluded from physical cache signatures; actual rest geometry is
still included. Linking does not introduce solver constraints. Shape edits remesh
and reset affected pieces through the existing transaction/cache contract.

Code: `authoring/linked.rs`, `atelier/drafting_ui.rs`, `atelier/measure.rs` and
the board interaction adapter. CPU coverage is in `tests/linked_mirror.rs`;
`CLOTHCRAFT_AUTHORING_SCENARIO=panel-tools` selects the opt-in native authoring
smoke, covering boundary ownership, a 200 mm ruler and a mirrored edit.

Validation: 164 CPU-suite tests plus the added sewing-direction regression, 43
CPU UI tests, warnings-denied all-target Clippy, and the 16-action native
`panel-tools` smoke pass. The native check confirms that selecting a boundary
while another panel is active chooses the correct owner without inserting a
boundary endpoint; it also checks the 200 mm ruler and linked point editing.
The separate 21-action native sewing/box/lasso workflow also passes (10 checks).
