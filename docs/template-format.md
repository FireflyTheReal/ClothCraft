# Sewing templates v1 and v2

The rectangular v1 rules below remain compatible. v2 adds polygon contours, segment names and Delaunay settings: see [meshing.md](meshing.md) and [first-fit.md](first-fit.md). Both versions use the same integrated sewing formulation. Later ordered-chain extensions are documented in [documents-and-sewing.md](documents-and-sewing.md).

Runnable examples: [two-panel.json](../examples/templates/two-panel.json) and [local-ease.json](../examples/templates/local-ease.json). Rust types with `deny_unknown_fields` are the current normative definition; compilation supplements deserialization with geometry/reference validation.

```powershell
cargo run --locked --no-default-features -j 2 --bin cloth-template -- validate examples/templates/two-panel.json
cargo run --locked --no-default-features -j 2 --bin cloth-template -- bake examples/templates/two-panel.json --output output/template-bake
cargo run --locked --release --no-default-features -j 2 --bin cloth-template -- simulate examples/templates/two-panel.json --steps 30 --threads 2 --output output/template-simulation
```

`example path.json` creates an example and refuses to overwrite a file. `bake` and `simulate` replace fixed-name artifacts in their output directory. `simulate` returns failure if a step did not converge; metrics and final state remain available for diagnosis. None of these modes opens a window.

## Panels and fabrics

The root contains `format: "marvelous-cloth-template"`, `version: 1`, `name`, `fabrics`, `panels`, `seams`. IDs are unique within each family: 1–80 ASCII alphanumeric, `_` or `-` characters. Current limits: 1–128 panels and at most 512 seams. Every `fabric` reference must resolve.

Each panel supplies a rectangular `pattern` (`width`, `height` in metres; `nx`, `ny` cells), `fabric`, `grain_degrees`, `placement` and optional `pins` (`top_left`, `top_right`, `bottom_right`, `bottom_left`). Dimensions: 0.01–100 m; resolution: 1–128 cells per axis. Resolution changes do not change seam definitions.

In pattern space, u points right and v down. Grain 0° corresponds to +u, 90° to +v; weft is perpendicular. Placement transforms `(u,-v,0)` with `R = Rx Ry Rz`, then adds `translation`. Angles are degrees, using column vectors. Pins lie at transformed corners.

`FabricProperties` contains `warp`, `weft`, `shear` (N/m), `bend_warp`, `bend_weft` (N·m), `density` (kg/m²), `thickness` (m), `compression_ratio` (0..1) and `compression_transition` (Green strain). Example parameters are uncalibrated. Grain is not a shared fabric property.

## Boundaries and seams

Boundaries follow the contour: `top` left→right; `right` top→bottom; `bottom` right→left; `left` bottom→top. `range: [a,b]` is a normalized rest-arclength interval, `0 <= a < b <= 1`, with extent at least 1e-9. These are not vertex indices.

```json
{
  "id": "centre-seam",
  "a": {"panel": "left", "side": "right", "range": [0.0, 1.0]},
  "b": {"panel": "right", "side": "left", "range": [0.0, 1.0]},
  "reverse_b": true,
  "notches": [[0.5, 0.5]],
  "stiffness": 5000.0,
  "allow_ease": false
}
```

After trimming the intervals, `reverse_b` reverses B. Each `notches: [[s,t],...]` pair maps A to B in these local coordinates, **after** B reversal. Endpoints `(0,0)` and `(1,1)` are implicit; interior notches must increase strictly in both components. The correspondence `t = phi(s)` is piecewise linear.

For example, `[0.4,0.6]` distributes unequal lengths around the notch. Unequal global/local lengths require `allow_ease: true`. This flag permits the correspondence and emits a warning; it does not resize rest geometry or generate a physical gather construction. It also does not represent body/garment ease from the EASE research.

## Sewing energy

For trimmed lengths LA and LB:

```text
g(s) = xA(s) - xB(phi(s))
dmu = (LA ds + LB dphi)/2
E = k/2 integral |g(s)|² dmu
```

`k = stiffness` is N/m². The average measure treats both sides symmetrically; at equal lengths with uniform correspondence, it becomes boundary length. The partition combines both boundaries' vertices and notches. Two Gauss points per interval exactly integrate the quadratic energy of the linear connection. Total stiffness therefore does not artificially grow with vertex count.

A sample has `g = sum(c_i x_i)`; repeated indices merge and `sum(c_i)=0`. Gradient: `k w c_i g`. Hessian: `k w c_i c_j I`, positive semidefinite. Energy is objective; internal forces and torques balance. This is a connection penalty, not a hard constraint, Lagrange multiplier or explicit yarn stitches.

Maximum gap is evaluated at all partition endpoints, where the norm of a linear gap reaches its maximum. RMS uses the same measure as energy. Relative length-comparison tolerance is 1e-8, not a manufacturing tolerance.

## Artifacts and initial scope

- `patterns.svg`: rest view, grain, colored boundaries and notches; no scale-correct printing or seam allowance.
- `cloth.obj`: simulated positions and per-panel groups; `vt` holds local coordinates in metres.
- `fibers.json`: one entry per triangle, world warp/weft directions, stretches, shear cosine and validity; order matches OBJ faces.
- `grain-guides.obj`: two central lines per panel, interpolated on deformed surface triangles; no added yarn mechanics.
- `template.json`: editable definition copy. Simulation also writes `initial.obj`, `metrics.jsonl` and `run.json`.

The initial v1 CLI handled rectangles only, with explicit placements, no avatar or inter-panel contact; even converged assemblies could self-intersect. Polygon/Bézier editing, avatar fitting and contact were added later and use separate documented paths. Darts, seam allowance, industrial pattern semantics, zippers, physical seam folds and explicit yarn mechanics remain outside this template foundation. See the current [roadmap](../ROADMAP.md) instead of treating this historical scope as the current editor feature list.

## Optional curve and board fields

A polygon may now include `"curves": {"edge_name": [[c1_u, c1_v], [c2_u, c2_v]]}`. Coordinates are local material meters; the edge's existing consecutive anchors remain its endpoints. Missing entries are straight. Unknown edge names and non-finite controls are rejected. Curves are flattened to 0.25 mm tolerance before meshing, then sampled segment chains are rejoined under the original edge ID. Holes and limited contour insert/delete editing were added later; see [drafting tools](drafting-tools.md) and [authoring](authoring.md).

A panel may include `"layout_2d": [x, y]`, the board location of its local material origin in meters, and `"layout_rotation_degrees": 30.0`, the board rotation (clockwise in its downward-positive screen coordinates). Display uses `origin + rotation(local_uv)`. Both fields are presentation metadata, excluded from physical signatures; neither changes material rest geometry or grain. Omitted layout is arranged deterministically beside preceding pieces. The angle defaults to zero, must be finite, and is omitted at zero to preserve historical state-cache signatures. Old files omit these fields and remain valid. New files with these extensions require this editor version; older strict readers may reject unknown fields.
