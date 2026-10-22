# Net material quantities

`garment::production_metrics::report(&template)` is a read-only CPU report of
net pattern area and cloth mass, per panel, per shared fabric and in total.
It compiles the editable template afresh, so a changed density or rest contour
cannot accidentally be paired with an old mesh/mass cache. No simulation state
is accepted, and no solver or document setting changes.

The JSON format is `clothcraft-net-material-metrics`, version 1. Areas are in
square metres, masses in kilograms, densities in kilograms per square metre
and panel-local material bounds in metres. Each authored panel counts once;
a linked mirrored panel is a separate piece. Fabric rows include only used
fabric IDs, sorted by ID. Equal display names do not merge different fabrics.

Area is the sum of compiled rest-triangle areas. Holes therefore contribute no
cloth; internal sewing paths do not remove material. Curved outlines use the
compiler's existing piecewise-linear curve sampling, not an exact analytic
curve integral. Mass comes from the same compiled lumped masses used by the
solver. Panel bounds describe local material axes, not optimized roll placement
or the garment's 3D extent. Board layout, grain and rigid placement do not
change these quantities.

This is **net cloth accounting**, not fabric to purchase. There is no inferred
seam allowance, grading/cut multiplicity, marker nesting, roll width, waste,
shrinkage, trim/thread weight or price. Manufacturing margins and allowances
must be modeled explicitly by future tools. Non-finite or overflowing report
quantities fail instead of producing JSON nulls.

The implementation is original, based on ClothCraft's existing material and
mesh definitions. No EXO source was copied or translated. The first checks use
original small fixtures: exact rectangles with nonmatching mesh densities,
a polygon hole, a curved ellipse, distinct fabrics sharing a display name,
density changes, placement invariance and invalid/overflowing inputs.

```powershell
cargo test --locked --no-default-features -j 2 --test production_metrics -- --test-threads=2
```

CLI integration: `cloth-template metrics <template.json>` prints this report;
it does not load an avatar, initialize a GPU or advance a simulation.
