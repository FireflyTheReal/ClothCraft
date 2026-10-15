# Pattern drafting UX: a design rationale

Owner: `ui-editor` (task-17, fashion-drafting UX). Status: written before the
implementation it describes, then kept in step with it. This is a rationale, not
a changelog: it states the intent behind each decision, what the current board
makes hard, what changes, and how the change is verified.

The person this is for has told us, in their own words, that the drawing process
"sucks" and that **they are not a designer**. So the standard applied here is not
"does the feature exist" but: *can someone who has never drafted a pattern produce
a correct piece and know that it is correct.*

## How to read this document

Every decision below answers four questions:

| Section | Question |
|---|---|
| **Intent** | What is the person actually trying to achieve at this moment? |
| **Why it is hard today** | What in the current board blocks or misleads them? |
| **Change** | What we change, and what we deliberately do *not* change. |
| **Verification** | How we know it works — a unit test where the claim is arithmetic, a stated interaction test where it is behavioural. |

Claims about the current implementation are read from the tree at the revision in
which this round started, by symbol rather than line number; `docs/CODE-QUALITY.md`
explains why line numbers drift here.

## Sources, and the limits of those sources

The studied public pages are the ones already recorded in
[the manual-study ledger](manual-study.md#authoring-backlog-review-2026-10-14).
Relevant to drafting:

- **MD-01 · Add Point/Split Line** — insert a point by clicking or by specifying a
  length, ratio or equal subdivision count. This is the direct source for
  "insert a point on a segment", and the ledger already assigns it to DRAW-02.
- **Polygon** — build a contour with successive points, close it, cancel points or
  the whole operation. Source for the drawing/undo behaviour we keep.
- **Edit AI Bézier Curve** — editable curve handles and conversion between
  straight and curved boundaries. Source for corner↔curve.
- **Segment Sewing (2D)** and **3D Segment Sewing** — pair boundaries, inspect
  direction and **length difference**. Source for why a length readout is not
  decoration: the seam decision *is* a length decision.
- **Move/Rotate Pattern with Gizmo** — directional manipulation with a numeric
  alternative. Source for "move with the mouse, correct with numbers".
- **W3C SVG path specification** — the cubic representation used for curves.

Three honest limits, stated because the register demands it:

1. The studied pages describe **Marvelous Designer behaviour**, not a pattern
   standard. Resemblance is a workflow signal, not evidence of algorithm parity.
2. **Seam allowance, notches, points of measure, grading and print calibration
   are not covered by the pages read so far.** The ledger says so explicitly:
   darts, trims and the rest of the manual "remain a future reading queue". Where
   this document relies on those concepts it labels them as **domain practice we
   have not yet sourced**, and proposes the reference pass that would fix that.
   None of it is claimed as implemented parity.
3. Nothing here is a new solver or model capability. The physical contract —
   material metres as the rest configuration, named boundaries, immutable compiled
   topology — is fixed by [AGENTS.md](../AGENTS.md) and
   [authoring.md](authoring.md), and this document only consumes it.

## The acceptance test we are committing to

Stated first, because everything below exists to satisfy it:

> **A person who has never drafted a pattern can, without reading any
> documentation: (1) create a valid four-point piece; (2) snap a corner onto a
> grid intersection; (3) fix one edge to an exact millimetre length by typing it;
> (4) move a point; and (5) undo each of those actions.**
>
> Secondary, same standard: they can state the perimeter of what they have drawn,
> and the length and angle at the corner they are editing, while they are still
> drawing it — not after they release the mouse.

"Without documentation" is a hard requirement, not a flourish. Every affordance
below must be visible **on the canvas, before the action**, and its effect must be
legible **while** the action happens. Nothing may depend on a shortcut the person
cannot see, or on a status string that appears after the fact.

---

## What shipped in this round

This rationale is kept in step with the implementation. As shipped, the board
(`src/atelier/board.rs`) provides:

| Affordance | Where it is visible |
|---|---|
| Snap kind and the captured coordinate | marker + chip beside the cursor, **before** the click |
| Snap suspended / angle locked | `Free · snap off` and `Angle 90.0°` chips while the modifier is held |
| Grab any point of any piece | emphasis dots on every point in Edit; no prior panel selection |
| Move a selection | orange rings on the selected points + `N points selected` |
| Live length, delta, corner angle | beside the cursor during the drag |
| Segment lengths and running perimeter | on the draft while drawing |
| Exact target length and deviation | `Target … mm` row with `Set exact length` / `Clear target` |
| Insert a point | green `+` marker with `Insert point on <boundary> · N mm` |
| Delete a point | red `×` badge labelled `Delete point (Del)`, a row button, and Del |
| Corner ↔ curve | `Round this corner` / `Make this corner sharp` in the selection row |
| Boundary dimension | name + true arclength (+ chord when it differs) on the selected boundary |

Verified without a GPU by `cargo test --locked --features desktop --bin cloth-fit`
(board unit tests) and `cargo test --locked --features desktop --lib render::`; the
command layer's own tests live in `tests/drawing_commands.rs` (DRAW-01/02).

---

## Decision 1 — Snapping

### Intent

Place a point where it *belongs*, not where the hand lands: a corner on another
corner, a midpoint on a midpoint, an edge on the grid, an edge exactly
perpendicular. In drafting this is not a convenience, it is how two pieces are
made to agree later: a 400 mm edge that is really 400.6 mm becomes a seam that
does not close, or that closes with unwanted ease.

### Why it is hard today

- The magnet is a single global grid: `Board::snapped` rounds to `snap_mm`
  (10 mm default) and is the only snap that exists. There is no endpoint,
  midpoint, angle or on-segment snap, so aligning to an existing point means
  getting within 10 mm of it by eye and hoping.
- Nothing tells the person what the snap did. The captured point silently differs
  from the pointer position, and the only feedback is the drawn point itself.
- The capture threshold is inconsistent: point hit-testing is in screen pixels
  (`< 8.` px for anchors), while the grid snap is in **material** millimetres. At a
  large zoom the grid is coarse in screen terms; zoomed out it is finer than the
  hand can resolve. The feel therefore changes with zoom, which is exactly the
  complaint the navigation round already fixed for the camera.
- Dragging an existing point runs the *absolute* pointer position through the grid
  snap, so a dragged point can only land on 10 mm intersections: fine adjustment
  is impossible while the magnet is on, and there is no way to suspend it.
- The modifier that would suspend snapping does not exist.

### Change

One detector, `resolve_snap`, evaluates candidates in a fixed priority order and
returns **both** the resulting material point and a label describing *why*:

| Priority | Snap | Meaning |
|---|---|---|
| 1 | `Point` | an existing contour point, including the point being drawn from |
| 2 | `Midpoint` | the midpoint of a straight segment |
| 3 | `On edge` | the perpendicular foot on a nearby segment, clamped to it |
| 4 | `Angle` | locked to 15° increments (which includes 0/45/90°) from the reference point |
| 5 | `Grid` | the drafting grid (`snap_mm`) |
| — | `Free` | no candidate within range; the pointer position is used |

- **All capture radii are screen pixels** (10 px for points and midpoints, 8 px for
  on-edge, grid-as-pixels derived from `snap_mm` and the current scale). The
  resulting *material* coordinates are computed from those pixels, so the magnet
  feels identical at every zoom — the same promise the camera now keeps.
- **The active snap is shown before the click**: a marker at the captured position
  plus a chip next to the cursor — `Point · 0.0 mm`, `Midpoint`, `On edge ·
  123.4 mm`, `15°`, `Grid 10 mm`. The chip states the measurement the person
  cares about, not the name of a code path.
- **A modifier suspends snapping**: hold the platform command key (⌘ on macOS,
  Ctrl on Windows/Linux) to place the point exactly where the pointer is. The chip
  reads `Free (snap suspended)`. The suspension is re-evaluated every frame, so it
  can be pressed and released mid-drag.
- **Angles are constrained by holding Shift**, which is the convention in drafting
  and CAD: Shift locks to 15° increments from the reference point, so
  perpendicular and 45° are reachable without a separate mode.
- The magnet checkbox controls the **grid step only** (`Snap to 10 mm`, driven by
  the `snap_mm` preference rather than a hardcoded 10 mm); the geometric snaps stay
  available with it off, and the command modifier suspends everything. That split
  is deliberate: the checkbox is a drafting preference, the modifier is a per-click
  decision.

We do **not** change: the grid remains the lowest-priority snap, so an endpoint
always wins over a grid line; and snapping never writes geometry by itself — it
only chooses the coordinate that a later, explicit click or drag commits.

### Verification

- Unit tests on `resolve_snap`: priority order (a point beats the grid and the
  on-edge foot; a midpoint beats the on-edge foot), screen-pixel invariance (an
  8 px offset captures, a 40 px offset does not, at 60, 300 and 2400 px/m), the
  suspension modifier returns the raw pointer coordinate, the magnet-off case
  returns `Free` while geometric snaps still capture, and Shift produces exact
  0/15/30/…/345° directions from the reference point.
- A stated interaction check for the visible part: hovering a grid intersection
  shows `Grid 10 mm` and the marker *before* any click.

---

## Decision 2 — Moving points

### Intent

Correct the drawing. This is the most frequent action in real drafting: the
outline is drawn approximately, then pushed into place. The person needs to grab
*that* corner and move *it*, immediately.

### Why it is hard today

- Moving a point requires a tool change **and** a prior piece selection: the anchor
  hit test only runs in `Tool::Edit` for the currently selected panel. In the draw
  tool, pressing on an existing point does not grab it — it **adds another point**.
  So the natural gesture is not merely unavailable, it is actively harmful.
- There is no coordinate feedback during the drag. The person lets go to find out
  where the point went, because the only readout is the pattern box annotation.
- There is no keyboard nudge, so a 0.4 mm correction requires pixel accuracy.
- There is no multi-select: correcting a whole edge means repeating the drag per
  point, and each repetition can introduce a new misalignment.

### Change

- **Grabbing never needs a tool change, and never destroys geometry.** In the edit
  tool, a press within 10 screen pixels of *any* displayed point grabs it — the
  piece and the point are selected in the same gesture, so there is no prior
  panel-selection step. In the pen tool, a press on a corner already placed moves
  that corner (the draft stays editable without leaving the pen), while a press
  near *another* piece's point deliberately does not grab it: it snaps the new
  corner onto that point, which is how two pieces are aligned. Selection follows
  the grab, so the point is visibly selected while it moves.
- **Live feedback during the drag**: a chip beside the cursor with the material
  coordinates to 0.1 mm (`X 123.4 · Y 456.7 mm · move +12.0 · -3.5`) and the
  segments attached to the point redrawn with their live lengths and the corner
  angle (Decision 3).
- **Arrow-key nudging**: 1 mm per press, 10 mm with Shift held, applied to the
  whole current point selection, one undo entry per press so an accidental nudge
  is one Ctrl+Z away. Keys act only when no text field owns the keyboard, which is
  the existing convention in this workspace.
- **Multi-select**: Shift+click toggles a point in the selection; dragging any
  selected point translates the whole selection rigidly. A committed or cancelled
  edit drops the selection, because a stale index would move the wrong point. The
  selected set is ringed in orange and the count is labelled (`2 points selected ·
  arrows nudge 1 mm (Shift 10 mm)`), so the person knows what a nudge will move
  before pressing it.
- Moves commit through the command layer's dedicated atomic commands
  (`MovePoints`, `Nudge`), which keep **identical boundary names and identical
  point count** — so identity cannot change as a side effect of moving.

We do **not** change: a drag without a modifier never silently becomes a
multi-point move, and moving points never remeshes or alters another piece.

### Verification

- Unit test for the selection lifecycle: a cleared preview drops the selection and
  its panel, so indices that a point-count change invalidated can never be used.
- Command-level tests for `MovePoints` and `Nudge` live in DRAW-01's file
  (`tests/drawing_commands.rs`), by agreement with `draw01`; the board does not
  re-derive their arithmetic.
- A stated interaction check: in the edit tool, pressing a point of an unselected
  piece selects that piece and starts moving the point in one gesture; in the pen
  tool, pressing empty canvas appends a corner while pressing a placed corner moves
  it.

---

## Decision 3 — Length and measurement feedback

### Intent

Know the size of what has been drawn, while drawing it. A patternmaker thinks in
millimetres and degrees: "this edge is 400 mm", "this corner is 90°", "the
perimeter of this piece is 1 200 mm". Length is also the input to the seam
decision, which the studied segment-sewing pages frame entirely around
**inspecting the length difference**.

### Why it is hard today

- While drawing, no segment length is displayed at all. Lengths appear only after
  the piece is meshed and selected, as the **pattern box** width and height — which
  is not the drawn contour: for a freehand polygon the box is not the outline, so
  the numbers on screen can be true of the box and false about the piece.
- The perimeter is never shown, in any state.
- Angles are never shown.
- Curved boundaries have a **true length** (the sampled material arclength, the
  quantity the mesher and the seam compiler use) and a **drawn chord** (endpoint to
  endpoint). The board shows neither, so the two are silently conflated. This is
  the distinction that decides whether a sleeve cap will ease into an armscye.
- There is no way to state an intent such as "this edge must be 400 mm" and have
  the tool tell the person how far off they are.

### Change

- **Live length on every segment while drawing and while dragging**, drawn along
  the segment at 0.1 mm precision, following the curve for curved boundaries.
- **Running perimeter** of the draft (while drawing) and of the selected piece's
  full contour (while editing), labelled `Perimeter 1 200.4 mm`.
- **Angle at the corner being edited**, between the two adjacent segments, in
  degrees to one decimal, labelled `Angle 90.0°`.
- **Persistent dimensions on selected boundaries**, offset outside the outline so
  they do not sit on the geometry, each labelled with the boundary name and its
  length. For a curved boundary the dimension reads the true arclength, and the
  chord is shown beside it when the two differ by more than the display threshold
  — `edge_2 · 412.6 mm true · 402.1 mm chord`.
- **Editable target length** for the selected boundary: a field the person types
  into, e.g. `Target 400.0 mm`, with a live deviation readout
  (`400.0 target · actual 386.4 · -13.6 mm`) that updates while the point is being
  dragged. Committing the target performs the exact-length edit described in
  Decision 5.
- The existing pattern-box annotation stays, but is labelled as the box
  (`Blouse · 400.0 × 600.0 mm box`) so it can no longer be mistaken for the
  outline's measurements.

We do **not** change: lengths are always computed from **material rest
coordinates**, never from the draped 3D shape, and never from screen pixels. The
one place screen pixels are allowed is the *hover* radius, which is an input
device property, not a measurement.

### Verification

- Unit tests on the measurement helpers: straight segment length, sampled
  arclength of a curved boundary versus its chord (they must differ for a bowed
  curve and agree for a straight one), perimeter of a closed contour, and the
  angle between adjacent segments at 90° and at 45°.
- A stated interaction check: drawing a rubber-band segment shows its length
  before release, in millimetres with one decimal.

---

## Decision 4 — Fully re-editable drawings

### Intent

Change the drawing after it exists: add a point where a seam will start, remove a
point that is not needed, turn a corner into a curve, and drag anything again —
without the drawing "breaking" and without losing the identity that the 3D view
and any seam depend on.

### Why it is hard today

- **Insert and delete do not exist.** The command layer deliberately refuses any
  edit that changes the number of points or the set of boundary names
  (`Named boundaries must stay stable`), because boundary names are shared with the
  sewing model and the 3D view. So the cap is real and correct; what is missing is
  the *migration* that would make the change safe, which is DRAW-02's work.
- **Corner↔curve exists but is hidden**: `Add Bézier handles` / `Convert to
  straight segment` live in the right-hand inspector, on a selected boundary, and
  nothing on the canvas suggests they exist.
- Re-dragging works, but only through the tool-and-selection precondition of
  Decision 2.
- Nothing on screen tells the person which boundary they are editing, so
  "identity is stable" is a promise they cannot see.

### Change

- **Insert a point on a boundary**: hovering a boundary in the edit tool marks the
  exact insertion point on the boundary and shows a clickable `+` badge just
  outside it, joined by a leader line and labelled with the boundary name and the
  material arclength (`Insert point on edge_2 · 123.4 mm`). The badge is offset on
  purpose: a click *on* the boundary must keep selecting the boundary, so insertion
  needs a target that cannot be confused with selection. Clicking the badge inserts
  one point through the command layer's transactional split; corners are excluded,
  so the badge only appears where a point can go. The new point becomes the
  selection, so it can be moved immediately.
  The projected position is on the *sampled* boundary (the same 0.25 mm
  subdivision the mesher uses), so on a curved boundary it can differ from the
  exact cubic by less than that tolerance; the arclength sent to the command is
  measured along that same sampling, so the inserted point lands where the badge
  promised.
- **Delete a point**: the focused point carries a small red `×` badge labelled
  `Delete point (Del)`, and the selection row carries a `Delete point` button with
  the same label. Both route through the command layer, which merges the two
  adjacent boundaries into one stable successor name.
- **An affordance is never offered for an edit that will be refused.** The delete
  badge, the delete button and Del are all disabled together, with the reason shown
  on the control (`A contour needs at least three points`, `<edge> is curved; make
  the corner sharp first`, `<edge> has a stored target; clear it first`, `A seam
  uses this point's boundaries; delete the seam first`). Likewise the insert badge
  is not drawn where the split would be refused: on a boundary carrying a stored
  target, or on a boundary a committed seam already uses. Clearing the target, or
  deleting the seam, brings the badge back. This is the same principle as the
  on-canvas validation note in the "What else is missing" list: say what will happen
  where the person is looking, rather than let a click fail silently.
- **A seam is a join, a target is a specification, and neither has identifier
  migration yet**, so both block a split or a point rejoin today — but they are
  different things to a patternmaker, and the reasons say which one is in the way:
  split the boundary and a seam's ranges would be undefined; delete the point and
  the two rejoined boundaries would change identity under a committed join. Fixing
  that properly is DRAW-02's range/identifier migration, not a UI decision, so the
  board states the block instead of working around it.
- **Corner↔curve is a canvas-row control**: the selection row offers `Round this
  corner` / `Make this corner sharp`, which curve or straighten **both** boundaries
  incident to the focused point, so a corner becomes a continuous curve.
  Corner→curve writes collinear controls at 1/3 and 2/3, so the outline does not
  move. Handles stay draggable as before, and the inspector's per-boundary toggle
  is unchanged.
- **Identity is shown, not promised**: the selected boundary's name travels with
  its dimension (`edge_2 · 412.6 mm true · 402.1 mm chord`), and the insert marker
  names the boundary it will split before the click.
- The draft cap of 64 points stays.

We do **not** change: boundary identity is never renumbered or renamed by a drag,
a curve toggle or a layout change; vertex and triangle indices are never persisted
in an edit or a file; and a rejected edit leaves the document exactly as it was.

### Verification

- Command-layer tests for split, delete and corner/curve are DRAW-02's
  (`tests/drawing_commands.rs`), by agreement with `draw01`: each is atomic,
  migrates or rejects seam references, and undo restores the previous names.
- Board-side tests cover the geometry the board owns: the snap and arclength
  arithmetic behind the insert marker, and the selection lifecycle. The
  insert/delete paths themselves are thin command calls, so there is no duplicated
  contour logic to test twice.
- A stated interaction check: hover a boundary, see `Insert point on … · 123.4 mm`,
  click, and confirm the new outline deviates from the previous one by less than
  the 0.25 mm sampling tolerance.

---

## Decision 5 — What "exact millimetre" means for a target length

### Intent

Type "this edge is 400 mm" and have the drawing agree, exactly, with the number
the person will measure on the fabric.

### Why it is hard today

There is no target-length concept at all, and the naive implementation is a trap:
"set the length" is ambiguous for a curved boundary, for an edge whose neighbours
must not move, and for a contour whose other edges have their own exact lengths.

### Change

One rule, stated on canvas and in the field's tooltip:

> Setting a target length **scales the selected boundary about its first anchor**
> until its material arclength equals the target. The first anchor does not move;
> the far anchor and (for a curved boundary) the two Bézier handles move with it.

- For a **straight** boundary this reduces to the obvious behaviour: move the far
  endpoint along the current direction until the length is exact.
- For a **curved** boundary it preserves the shape and the angle at the fixed
  anchor, and changes the far endpoint and both controls by the same factor.
- The rule is owned by the command layer (`Command::TargetLength`, DRAW-01), not
  re-derived in the board: the board sends the target in metres and reads the
  achieved length back, so the value is exact rather than a rounded copy of the
  contour. A target of `0` clears the stored target without moving geometry, so the
  control is reversible.
- Neighbouring boundaries therefore change length; the live perimeter and the
  neighbouring angles update as the person types, which is the honest consequence
  and is shown rather than hidden.
- An edit that would produce an invalid contour is rejected atomically with a
  visible reason; the document and the undo history are unchanged.
- The stored target and the live deviation are shown next to the field
  (`measured 386.4 mm · -13.6 mm from the stored target`), so the person sees
  whether the drawing currently agrees with what they typed.
- **A target is a stored specification, not a hidden constraint.** It survives
  save and reload, loading never enforces it, and storing or clearing one does not
  reset the drape — only enforcing a different length does. While a target is
  stored on a boundary, the operations that would invalidate it (splitting it, or
  deleting one of its points) are refused, so the board does not offer them there.
  A stored entry that is not a usable specification (unknown boundary, non-finite
  or non-positive value) reads as *no target* and is surfaced as an ignored entry
  in the selection row, never silently trusted.
- Numeric entry uses the shared millimetre field from the numeric-entry layer, so
  the board and the inspector format, parse and step millimetres identically.

### Verification

- Command-layer tests: `tests/drawing_commands.rs` (DRAW-01/02) covers the exact
  length edit, the clear-target case and atomic rejection; the board does not
  duplicate that arithmetic.
- Board-side tests: the measured length and the deviation come from
  `authoring::edge_length` and `authoring::deviation_mm`, exercised alongside the
  board's own chord/perimeter/angle math by the measurement-helper test.
- A stated interaction check: type `400` in the target field of a 386.4 mm
  boundary, press `Set exact length`, and confirm the readout and the dimension
  annotation both read `400.0 mm`.

---

## Invariants this design does not trade

Restated because every decision above touches the canvas where they are easiest to
break:

1. **2D material metres are the rest configuration.** Nothing in the drafting UI
   derives rest geometry from the draped 3D state, and no readout is computed from
   the drape.
2. **Boundary identity survives every edit.** Insert and delete migrate names
   transactionally or are rejected; drag, nudge, curve toggle and target length
   never change the name list.
3. **Layout and rotation stay presentational.** Board pan, zoom and rotation, and
   piece layout rotation, change `layout_2d` / `layout_rotation_degrees` only —
   never rest shape, grain, masses or cache signatures.
4. **Every mutation is one atomic, undoable edit** through `authoring::apply`; the
   board never writes the template directly.
5. **A rejected edit leaves the document untouched** and explains itself where the
   person is looking.

---

## What else is missing

Ranked by how much it costs the non-designer we are designing for, not by how
interesting it is to build. Items 1–4 are the ones I would argue for next; items 5
and beyond are the professional layer that turns a drawing tool into a pattern
tool. Everything here is a **recommendation**, including the parts the user did not
ask for.

1. **Seam allowance, and the difference between the sewing line and the cut line.**
   *Not sourced from the studied pages — this is domain practice, and it needs its
   own reference pass before we claim parity.* Every production pattern has two
   outlines: the sewing line (which is what ClothCraft models, and what the solver
   seams) and the cut line offset from it — commonly around 10 mm on knits, wider
   on wovens, narrow on leather — with corners either mitred or notched. Without
   it, the app produces a correct *shape* but not something a person could cut out.
   The SVG export says "not a print-to-scale sewing pattern" for exactly this
   reason. Recommend: a non-structural offset layer (the roadmap's DRAW-04 already
   separates guides from cuts), a per-fabric default allowance, and a verification
   that the offset stays attached and never crosses itself. This is the single
   biggest gap between "looks right" and "can be made".
2. **Notches and balance marks.** The seam compiler already carries notch
   correspondence, but there is no tool to *place* a notch, and no POM marks. A
   notch at a material arclength is how a sewer aligns a sleeve cap to an armscye
   without measuring; it is also how a mismatched pair is detected before sewing.
   Recommend: a notch entity on a named boundary at a material arclength, visible
   in both views, with the seam preview reporting notch alignment.
3. **Named measurements (points of measure) and a measurement chart.** The
   industry checks a garment against a *chart*: named measurements (chest, waist,
   sleeve length, hem sweep) defined between two landmarks, each with a target and
   a tolerance. The target-length field in this design is one edge; a chart is the
   generalisation, and it is what makes "is this the right size" answerable.
   Recommend: a table of measurements, each `{name, landmark A, landmark B, target,
   tolerance, actual, status}`, where a landmark is a named point or a
   boundary+arclength; live status against tolerance; CSV export. This is the
   highest-value item on this list for a non-designer, because it converts the
   drawing into a checkable specification instead of a shape.
4. **Symmetry and linked editing (DRAW-03).** Half-pattern drafting is normal
   practice — draft half a bodice, mirror it — and it halves the drawing work while
   removing a whole class of "the two sides do not match" errors. For someone who
   is not a designer, "draw one half, the other follows" is the largest single
   reduction in effort available. Already on the roadmap; I would rank it above the
   remaining sewing-chain work.
5. **Grading / size runs.** Real patterns are graded across sizes by rules applied
   to landmarks, not re-drawn per size. Recommend an incremental per-landmark
   `dx, dy` rule table attached to the template, with the rest mesh rebuilt per
   size. Until then, "make a medium from a small" is a manual redraw.
6. **A seam length budget (ease), not just a length difference.** The studied
   segment-sewing pages frame the check as a length difference; in practice a
   sleeve cap is *supposed* to be longer than its armscye by a controlled amount,
   and the direction of the difference matters. Recommend: a target ease
   (millimetres or percent) per seam, with the live actual-vs-target status shown
   where the pair is chosen, reusing the same deviation readout as the target
   length. This is the drafting decision the current "length delta" only half
   answers.
7. **A named, visible edit history.** Undo exists and is atomic, but it is an
   anonymous stack. A non-designer needs a list they can read — `Inserted point on
   edge_2`, `Set edge_3 to 400.0 mm`, `Moved 2 points` — with the ability to jump
   back several steps. The command layer already produces typed edits, so this is
   mostly presentation.
8. **Construction and reference lines.** Drafting is done on construction lines
   (centre front, bust line, hip line) before the outline is committed. Today the
   board has a grid and nothing else. Recommend non-structural guides that can be
   snapped to *and* hidden before simulation, so the reference geometry never
   reaches the physics.
9. **Print calibration and a tile check.** The SVG is explicitly a preview. A 1:1
   PDF with a printed 100 mm calibration square, and a warning when the printed
   scale is wrong, is what lets a person compare the digital piece with the fabric.
   This also belongs in the interchange plan.
10. **Teach the failure where it happens.** Invalid contours are rejected with a
    message in the status bar or a dialog. A non-designer needs the *offending
    geometry highlighted on the canvas* with a sentence about what to do
    (`This outline crosses itself near the top-left corner; move that point out`).
    Recommend: localize validation failures to the boundary or point that caused
    them, and treat this as part of the affordance rather than as error handling.
11. **A first-run path that is not a blank board.** Recommend a default piece with
    four named corners and a one-line prompt on canvas (`Click to place each
    corner, then close the shape`), plus a single visible "finish the shape"
    control. This is the cheapest possible improvement to the first thirty seconds,
    and it is the difference between "I can draw" and "I do not know what to do".
12. **Measurement units and format discipline.** Millimetres with one decimal is
    the drafting convention we are adopting; imperial drafting (inches, eighths)
    is still common in some markets, and a mixed-unit garment is a defect. If a
    unit switch is ever added, it must be per-document and visible, not per-widget.

Items 1–3 are the ones I would bring to the owner as "what the tool still cannot
do that a patternmaker would notice immediately". Item 11 is the one I would ship
first if the goal is only to stop the drawing process feeling hostile.
