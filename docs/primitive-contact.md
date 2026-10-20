# Finite-primitive contact experiment

`--primitive-distances` is an opt-in CPU contact-law ablation in
`solver_compare`, usable with the existing research solvers. It is not an IPC
backend, not Geometric Contact Potential, and not enabled in the editor.
GPU stepping rejects it. It cannot be combined with the earlier EE taper.

## Hypothesis and implementation

The [previous investigation](contact-refresh-follow-up.md) found finite
energy jumps when OGC's force admissibility changes between geometric
features. This experiment replaces that force selection with persistent
vertex–triangle and segment–segment pairs. At every energy, gradient and
Hessian evaluation it recomputes distance to the **finite** primitive.
Closest-point coordinates are differentiated within their selected region.
The activation, radius, pair stiffness and EE parallel mollifier remain the
existing ones; all neighboring primitives contribute independently.

This changes the contact law. In particular it removes the constructive
force-cone rejection and can add spurious repulsion between nearby material
points. Contributions are not area weighted and are not normalized across
incident primitives. Mesh refinement can change the aggregate force.

`ogc/primitive_distance.rs` contains the finite distances.
`Kind::PointTriangle` and `Kind::SegmentSegment` carry the new terms through
the existing incidence, coloring, VBD and full-primal machinery. No rest,
mass, seam, damping, predictor or timestep change is involved. Exact parallel
EE terms retain the existing zero mollifier contribution; geometric guards
remain necessary.

Scalar support checks skip automatic differentiation outside the contact
radius. Scalar energy evaluation classifies once, without that AD prepass.
Tests compare shortcut/full values, gradients and Hessians, including nearly
parallel edges and activation boundaries.

## Coverage without shrinking movement

Let the original geometric query radius be `q`, the contact radius `r`, and
the OGC relaxation factor `beta < 1/2`. Each vertex stays within an original
ball of radius at most `beta*q`, additionally restricted by directional
half-spaces where present. The force search retains pairs out to

```
R = max(q, r + 2*beta*q) + floating_point_reserve
```

A primitive pair initially outside `R` cannot enter the active shell while
both primitives remain within those original bounds: each convex primitive
moves by at most `beta*q`. Thus feature changes and the arrival of previously
unseen pairs do not require adding a positive-energy term during a valid
frozen solve. This is a force-coverage argument, not a new collision proof.

The original `q` still initializes radii and directional thresholds; the
larger radius is used only for candidate search and force retention. Extra
pairs beyond `q` cannot reduce its capped distance bounds. Tests compare
radii, half-spaces and minimum distances exactly. The numerical reserve
does not turn f64 predicates into exact arithmetic.

## Validation gates

- Both public EE and concave-PF fixtures retain their contact energy at
  refresh, with unchanged separation regions.
- A separate halo fixture moves a previously inactive pair inside the
  activation radius while staying in the original bounds. Its force was
  already present as a zero-energy term.
- Temporal cache reuse and fresh detection agree while query radii change.
- Finite-distance values agree with existing closest-geometry routines;
  derivative differences, rigid-transform invariance and force/torque balance
  are checked away from switching surfaces.
- A supported cloth slides, lifts under a grab and falls after release.
  Where the two laws coincide, all 120 complete states are byte-identical.
- `contact_slide_probe` samples a point over a **two-triangle** frictionless
  plane at a fixed true gap. This exposes force-law defects that the previous
  single-face floor test cannot detect.

See [measured results](primitive-contact-results.md) before selecting this
experiment. Passing intersection checks or removing refresh jumps is not
sufficient for promotion.

## Research consulted

[No Free Slide, §§4–5](https://arxiv.org/html/2308.01696) analyzes artificial
tangential forces from superimposed primitive potentials and explores smooth
IMLS surfaces. Its planar-slide experiment motivated our additional
falsification gate. We have not implemented its smooth surface model.

[Geometric Contact Potential, §§5.2–5.3 and Appendix A](https://arxiv.org/html/2402.00719v3)
uses directional factors, closest-point mollification and a continuum-based
discretization. Those factors depend on surrounding geometry; simply
weighting a four-vertex term without differentiating the neighboring vertices
would not implement that energy consistently. A cloth adaptation also needs
an explicit treatment of open, two-sided surfaces. This is a research lead,
not a claim that its full formulation is available here.

## Reproduce

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test primitive_contact -- --test-threads=2
cargo build --release --locked --no-default-features -j 2 `
  --example solver_compare --example edge_transition_probe --example contact_slide_probe
target/release/examples/contact_slide_probe.exe --output output/new-slide.json
target/release/examples/edge_transition_probe.exe `
  --primitive-distances --point-face --output output/new-face-transition.json
target/release/examples/solver_compare.exe `
  --project path/to/source.json --output output/new-primitive-run `
  --backend vbd --steps 120 --iterations 16 --primitive-distances --pull --audit-every-step
```

For the recorded 40-step comparison, use `--steps 40 --pull-start 5
--pull-steps 10 --hold-steps 5`, and run each of the four combinations of
`--primitive-distances` on/off and `--pull` on/off. The six-step refresh
diagnostic uses `--steps 6 --convergence` instead. For the static captured
transitions, add `--primitive-distances` to the existing
`edge_transition_probe --project ... --capture ...` command, with
`--point-face` for a PF witness. The CPU test suite remains independent of
private dress inputs.

Use a fresh output path for every run. The contact law is recorded in the
protocol; comparing its energy with the original law is not an equal-model
energy comparison. Compare refresh deltas within each law, geometry,
independent surface audits, release response and complete step cost.
