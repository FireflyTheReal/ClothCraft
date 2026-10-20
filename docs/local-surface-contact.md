# Local surface field: planar success, general contact still unproven

The CPU research kernel in `ogc::local_surface` replaces a sum of independent
primitive barriers with a barrier evaluated on a normalized local plane field.
It is not installed in VBD, the GPU path or the editor.

## Formulation

For oriented samples `(s_i, n_i)` with fixed positive quadrature coefficients
`a_i`, the prototype computes

`w_i = a_i max(1 - |p-s_i|²/R², 0)³`

`phi(p) = sum_i w_i [n_i · (p-s_i)] / sum_i w_i`.

The approach is motivated by [Larionov et al., §4.1](https://elrnv.com/fcses_tog21.pdf),
which blends local implicit fields with normalized weights. Their §7 explicitly
excludes self-contact and identifies neighborhood selection in creases as an
open issue. This implementation uses its own compact polynomial in squared
radius and the existing OGC activation, not the paper's complete contact and
friction solver.

Triangle samples use differentiated centroids and unit normals. Fixed reference
weights do not change with current deformation. The field has units of metres
but is generally **not a Euclidean distance to the triangle mesh**. A barrier
uses a caller-selected fixed side, `b(side * phi)`; the side is never chosen
again from the sign of a line-search trial.

Empty support, exhausted numeric weights and a crossed oriented field return
errors. They do not silently disable contact. Common weight scaling cancels
analytically; rescaling before division and anchoring the weighted plane sum
improve floating-point conditioning while preserving all derivatives.

The kernel is twice differentiable across a sample's support boundary while
other samples remain. Loss of the final sample is outside that statement.
Normals, centers and spatial weights contribute to differentiation; no normal freezing,
extra damping, sleep, velocity clipping or mesh-rest changes are introduced.

## Flat sliding and release

Eight scans use rotated planes with 8 or 128 triangles, regular/uneven spacing,
alternating diagonals and support radii of 80/200 mm. At 1 mm height and a
reference normal force of 225 N, maximum measured errors are:

- field value: below `9e-18 m`;
- lateral force: below `1.32e-13 N`;
- normal force: below `2.02e-12 N`.

These are rounding-scale errors. The active sample count changes during the
scans without producing the force ripples of the preceding two-sided trial.

The existing fixed-patch headless laboratory also tests support radii
50/80/200 mm, each with release in motion and release after holding.
All six runs complete 480 steps at a 1/60 s simulation timestep, with zero
unconverged steps and no line-search failures. Positions and velocities match
the analytical frictionless-plane control exactly at the recorded steps.
The moving release retains 4 mm/s; the held release remains at rest.
Minimum separation is 1 mm and measured post-release energy rise is zero.
Original OGC geometric guards remain active throughout.

This is a useful local contact result, not a solved dress-jitter claim.

## Collision counterexamples

The tests deliberately retain failures that a planar benchmark would miss:

1. Two parallel, equally oriented sheets 2 mm apart blend into a fictitious
   zero-set halfway between them. Normal coherence is exactly one, so a
   normal-alignment threshold cannot identify this error.
2. Oppositely oriented sheets can yield zero query gradient at their midpoint,
   even though the field value is nonzero.
3. On a concave crease, the field is about 1.804 mm at a point lying on an
   actual face. Its barrier is finite there (`0.071535 J` in the test).
   The reconstructed zero-set therefore cannot replace the original mesh's
   collision guards.

Known separate plane patches reproduce the analytical two-plane barrier sum.
This positive control requires explicit correct patch identities; it is not
an automatic separation algorithm for folded cloth.

## Cost and scope

The frozen dress at 49.45 s was also inspected without stepping. For each of
225 active production VF terms, the audit builds the target face's edge-neighbor
patch, retaining the world's original seam/topological exclusions. It uses unit
sample weights and a per-query frozen support radius twice the sum of the
largest sample-center distance and the contact activation radius. This is a
diagnostic recipe, not a temporal patch/radius selection rule.

The maximum absolute gap error is **1.665 mm**; 173 queries exceed 0.1 mm.
At a point on the seed triangle itself, the largest absolute field value is
**1.734 mm**, instead of zero. For 199 queries, the absolute field lies outside
the 3 mm activation band even though the original contact term is active.
Many original terms are near activation, so this count is not a count of deep
penetrations or failed trajectories. It does show a substantial change in
which forces would be applied. The current production contacts, centers,
radii and directional bounds compare equal before and after this read-only audit.

This falsifies this neighborhood recipe as a direct replacement for the
production law. It does not rule out other implicit surface constructions.

The development-build microbenchmark measures a cached-sample Jet3 query
(value, query gradient and Hessian), including local kernel allocations.
Single observations range from about 3.4 to 71.2 microseconds across 8/128
samples. Sample construction, search, target-vertex derivatives and the cloth
solve are excluded. These numbers are not frame times or a 60 Hz claim.

The next requirements are stable surface-patch ownership, support continuity
under motion, conservative compatibility with the actual triangle surface,
and matched full-dress pull/release. The production contact law remains unchanged.

Validation covers 49 distinct targeted CPU tests across the base and follow-up
runs, including the retained negative controls. CPU/desktop all-target Clippy,
formatting and tracked-diff checks pass. The final probe reproduces the initial
static accuracy measurements (microbenchmark timings excluded). The user's
garment file hash is unchanged.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test local_surface --example contact_patch_probe -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example local_surface_probe -- --output output/local-new/static.json
cargo run --locked --no-default-features -j 2 --example contact_patch_probe -- --local-support 0.05,0.08,0.2 --output output/local-new/pull.json
# Optional read-only VF neighborhood audit:
# add --project <portable-project.json> to local_surface_probe.
```

Evidence: `output/local-surface-20261020`.
See [validation](local-surface-validation.json) for source and result hashes.
