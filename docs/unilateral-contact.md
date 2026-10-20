# Unilateral contact: a local QP experiment

The isolated contact tests pass with a working set of inequalities. This is a
research kernel, not a change to the production VBD/OGC solver and not a fix
validated on the dress. It uses a **different contact law**: explicit hard
clearance instead of the existing barrier energy.

The reason to test it is the measured energy jump when the current contact
representation changes. A hard constraint has no stored barrier energy to
appear or disappear at feature refresh. Its reaction is a solver unknown.
Duplicating a binding constraint can change individual multipliers, but should
not change the converged displacement or total reaction.

## Implementation

`src/solver_lab/contact_qp.rs` solves the three-dimensional convex problem

```text
minimize  0.5 dᵀ H d + gᵀ d
subject to nᵢ · d >= bᵢ
```

The metric must be symmetric positive definite. A Cholesky solve retains
off-diagonal coupling. Dual coordinate ascent projects each multiplier onto
the nonnegative half-line and updates the complete primal displacement.
Normalizing a row also normalizes its offset; returned multipliers are forces
in newtons along unit normals.

The report checks feasibility in metres, stationarity in newtons and
complementarity in joules. Exhausting the iteration budget returns
`converged: false`. No diagonal shift, compliance, force clipping or damping
is inserted. Warm starts are optional; matching row identities is the
caller's responsibility.

This is an independent small QP implementation. It is not an implementation
of full AVBD, XPBD, Box3D or GCP. The inequality/contact discussion in
[Small Steps in Physics Simulation](https://mmacklin.com/smallsteps.pdf)
and the [AVBD project](https://graphics.cs.utah.edu/research/projects/avbd/)
are relevant context, not validation of this implementation.

## What the tests exercise

Six kernel tests cover duplicates and scaled rows, released reactions,
opposing constraints, infeasibility, invalid input and a non-diagonal metric.
Forty seeded feasible problems are compared with an independent dense
active-set oracle, including reversed row order.

`examples/unilateral_contact_probe.rs` compares:

1. An analytical plane constraint.
2. Every finite primitive's tangent inequality at every iteration.
3. A working set: start with near-active constraints, solve, and add any
   omitted primitive whose true swept clearance would be violated.

The two fixtures are a point above a triangulated plane and a transverse
rigid wire crossing the junction between collinear target edges. The second
uses actual finite segment/segment closest points. It is a translating wire,
not a deforming cloth patch.

Both retain the original OGC displacement guards. Every accepted path is
also checked against the finite primitives at 1 mm clearance. Point/triangle
checks include the segment endpoints, triangle edges and intersections.
For the transverse wire fixture, the exact swept distance reduces to the
centre-path/target-segment distance; the whole wire remains above the guarded
plane. **This is not general moving-triangle or deforming-edge thickness CCD.**

Each run has 480 steps at 1/60 s, mass 0.02 kg, a 225 N normal load and a
scripted grab. One protocol releases during translation; the other holds
before releasing. There is no friction, sleep, velocity damping or position
filter. The geometry is refined and constraint rows are duplicated eight
times. Three example tests check swept paths and moving/held EE release.

## Results

See [machine-readable results](unilateral-contact-validation.json). Raw
trajectories are local, under `output/unilateral-contact-20261020/`.

| Gate | Working-set result |
| --- | --- |
| Completed runs | 16 × 480 steps, plus four analytical references |
| Unconverged accepted steps | 0 |
| Maximum position difference from analytical reference | 1.733 × 10⁻¹⁰ m |
| Maximum velocity difference | 1.035 × 10⁻¹⁰ m/s |
| Minimum recorded swept clearance | 1 mm |
| Largest post-release energy increase | 4.72 × 10⁻¹⁶ J, floating-point scale |
| Moving release | Tangential speed remains approximately 0.004 m/s |
| Held release | Returns to rest within the test tolerance |

The moving-release result is essential: without friction, continuing to slide
is correct. A method that freezes this motion would fail even if its jitter
score looked better.

The all-linearized control fails **all sixteen runs** near the shared-feature
transition, after 118 or 119 completed steps. Its 4,096-sweep inner QP budget
does not meet complementarity. A neighbouring primitive's tangent half-space
can impose unnecessary restrictions even when the actual finite geometry
permits the proposed slide. These incomplete trajectories are retained as
failures, never included as successful release runs.

## Limits and next gate

The hard clearance is 1 mm; the unchanged OGC guard radius is 3 mm. Those
numbers have different roles. The probe is not a matched-law performance
comparison with the production barrier. Recorded wall time is for a small
CPU development-build experiment, not cloth FPS.

No elastic energy, sewing, deforming contact targets or whole-garment
convergence is tested by this probe. A per-vertex constrained QP can also lock
a mode that needs several vertices to move together. Before any production
integration:

- Couple both sides of contact with membrane/sewing variables and check the
  full primal/dual residual, not merely small position updates.
- Handle an initially infeasible clearance explicitly. Never silently
  reposition a saved dress or choose thickness per troublesome vertex.
- Retain original collision detection, seam exclusions and OGC bounds; add
  genuine deforming-feature swept-clearance checks where required.
- Repeat prestressed, folded, narrow-gap and pull/hold/release cases before
  replaying the frozen dress and measuring complete step time.

The current solver, material rest geometry, contact law and GUI remain
unchanged. No GPU run or new dress trajectory was performed in this pass.

## Reproduction

Validation included 32 targeted CPU tests (the new kernel/probe plus existing
OGC, feature-transition, primitive-contact and energy-accounting regressions).
The nine kernel/probe tests were rerun after adding explicit overflow rejection
inside the dual update. No GPU tests were run.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test contact_qp --example unilateral_contact_probe -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example unilateral_contact_probe -- --output output/unilateral-contact-20261020/results.json
python scripts/summarize-unilateral-contact.py output/unilateral-contact-20261020/results.json --output docs/unilateral-contact-validation.json
```
