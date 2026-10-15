# Firefly authoring integration — 15 October 2026

This is a selective adaptation of MIT-licensed contributions from
[FireflyTheReal/ClothCraft](https://github.com/FireflyTheReal/ClothCraft), authored
by `julienfoss`, through commit
[`b2cb17f71c8c908bc2088812d45bfbce6128eebe`](https://github.com/FireflyTheReal/ClothCraft/commit/b2cb17f71c8c908bc2088812d45bfbce6128eebe).
The common base is `536dbe5`. It is integrated with our drafting commit `b241505`.
Attribution does not imply that the contributor reviewed this integration.

## Included

- Numeric rectangle resizing and anchor coordinates, point selection/nudging,
  contour splitting/removal, curve conversion and persisted target lengths.
- Point/midpoint/edge/angle/grid snapping and live dimensions.
- 3D camera presets, orthographic projection, framing and cursor-centered zoom;
  board view rotation, zoom and framing.
- English interface, light/dark themes, persisted startup preferences, original
  vector glyphs (now connected to the atelier tool buttons), and the legacy editor shell.
- CPU-only desktop CI and a cross-platform build workflow. Hosted platform
  results must be checked on the exact published commit; local Windows checks
  do not qualify Linux/macOS releases.

The relevant upstream changes are `78f0f93`, `83ee45f`, `16f6fc4`, `db46440`,
`5913e6f`, `40a6bed` and `b2cb17f`; only selected source/tests/workflows and the
authoring guide are taken. The Newton block solver, CLO binary-derived research
catalogues and macOS app-bundle script are **not included**. The current physics
and public-manual research policy are unchanged.

## Integration fixes

- Preserve holes, internal sewing paths, mirrored copies, booleans and ruffles in
  the shared model, board hit-testing, pen modes and inspector.
- Keep the grabbed anchor in board coordinates during group dragging; convert
  board rotation from radians to degrees for arrow nudges. Preserve the clicked
  grip when dragging another member of a selection.
- Shift-click toggles point membership; repeated nudges retain the selection.
  Topology changes invalidate point indices. Boundary selection clears stale
  point selection, and point-edit shortcuts are limited to Edit mode.
- Committed point moves translate attached Bézier controls just like previews.
  Numeric rectangle resizing also scales holes, internal curves and material pins.
- Mirror copies retain target specifications under stable boundary names.
  Booleans reject operands with targets rather than silently discarding them.
  Splitting sewn or targeted contours requires removing the relation first.
- Store target lengths as nonphysical metadata: loading never enforces them,
  and changing only a target does not invalidate a saved simulation cache.
- Share one target-length draft between the board and inspector.
- Theme and snap preferences apply immediately. Simulation/overlay defaults are
  explicitly labelled for the next launch. Unsupported autosave and mesh-default
  preference controls were removed; existing project export/remeshing remain.
- Normalize test path separators and sort directory entries for cross-platform
  fixture comparisons without weakening content checks.
- CI validates the already-built executable and includes the CC0 mannequin,
  its notices, templates and license in the downloadable build artifacts.

## Verification and remaining scope

`tests/drafting_integration.rs` checks combined domain/point operations and the
board unit tests check rotated dragging, arrow directions and selection.
Use `scripts/check-cpu.ps1`, then desktop `cargo test` and `cargo clippy` with two
jobs/test threads. These checks do not initialize a GPU; GPU tests remain ignored.
An actual native interaction/draping pass still needs explicit GPU availability.

Local Windows validation on 15 October 2026:

- `scripts/check-cpu.ps1`: passed (formatting, headless tests, Clippy and five
  template validations).
- `cargo test --locked --features desktop -j 2 -- --test-threads=2`: **165 passed,
  0 failed, 3 GPU tests ignored**, including the final saved-state/target metadata
  regression added after the headless pass.
- `cargo clippy --locked --features desktop --all-targets -j 2 -- -D warnings`:
  passed without warnings.

No GPU benchmark or native window interaction was run for this integration.

Point selection is within one panel, not multi-panel selection. Internal paths
and hole rims remain sewable, but the new split/target tools address outer
contours only. File dialogs, autosave/recovery, partial seams, linked symmetry,
hole-point tools and production packaging are still unfinished.
