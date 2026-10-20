# Two-sided contact trial: not promoted

This pass implements an independent CPU contact energy and tests its force,
not just its candidate coverage. It fails the flat sliding gate and must not
replace the editor's VBD/OGC contact law.

## Model and implementation

`ogc::two_sided_contact` evaluates each VF/EE/VE/VV pair as

`weight × feature_mollifier × minimum_factor_A × minimum_factor_B × barrier(distance)`.

The [GCP paper](https://arxiv.org/html/2402.00719v3), §5.3.3 and Appendices C/D,
motivates the local-minimum directional factors: face closest points require
no extra test, edge points check incident face interiors, and vertices check
incident tangent directions. The cubic cumulative transition has zero first
and second derivatives at its support endpoints.

This trial deliberately omits the oriented-volume exterior criterion, accepts
both sides, and uses the existing dimensioned OGC activation. It does not
implement the paper's full model, adaptive rest localization or solver.
The results below are **not a falsification of published GCP**.

All directions and closest-point displacements are differentiated, including
the neighboring vertices outside a pair's usual stencil. VE tests differentiate
12 coordinates; EE tests differentiate 18. Finite differences check gradients
and Hessians. Additional tests cover translation/rotation, normal reflection,
face orientation, total force/moment and cumulative-cubic knots. A production
integration would need extended incidence and conflict coloring.

Pair weights are explicit fixed dimensionless coefficients. The isolated
experiments compare unit weights with reference feature measures: vertex
mean-edge-length squared, edge length squared, and triangle area, each divided
by a fixed reference area of 0.02 m². Full patch pairs multiply both feature
weights; the isolated point has weight one. These are two stated experimental
choices, not calibrated materials. The synthetic reference geometry is fixed;
no garment rest configuration is changed.

## Flat-patch force failure

An isolated point is swept at 1 mm height across the internal diagonal of a
flat two-triangle patch. The analytical plane has zero lateral force and
225 N normal force under this test's barrier stiffness. The scan spacing is
1 µm; the feature width is 0.01.

| Alpha | Unit-weight peak lateral force | Reference-measure peak lateral force |
| --- | ---: | ---: |
| 0.05 | 8179 N | 35648 N |
| 0.20 | 1163 N | 7038 N |
| 0.50 | 2058 N | 3345 N |
| 0.80 | 2160 N | 2008 N |

Energy remains positive and the force approaches a continuous limit at the
shared edge, but neither property prevents large force ripples. At alpha
0.05, minimum normal force is approximately -123 N with unit weights and
-660 N with reference measures: part of the scan is attractive.
No force clamping or derivative omission is used.

To check that this is not only an isolated-point effect, a second test slides
a two-triangle surface patch above a larger flat patch. All 117 eligible
candidates are retained throughout; there is no detector refresh. VF, EE and
VE terms become active. Two source orientations (0 and 0.37 rad) and alpha
0.2/0.5 produce peak lateral forces of 218–526 N with reference measures,
and 4141–8291 N with unit weights. Reference-measure normal force spans
roughly 8.57–144.71 N across the cases. Fixed-height energy varies with sliding.
This remains unsuitable for a frictionless plane.

## Pull/release failure

The existing headless 60 Hz fixed-patch laboratory runs both release while
moving and release after holding, with the original OGC movement guards.
Four alpha values and both weight choices produce 16 candidate runs, alongside
10 existing analytical/patch controls. The solve budget and residual criterion
are unchanged. No additional damping, freezing or velocity modification is used.

All candidate runs retain positive separation (minimum across runs: 0.187 mm).
All nevertheless have unconverged steps: 66–364 per run. With alpha 0.05,
moving-release speed falls essentially to zero instead of retaining the
analytical 4 mm/s. Other choices drift sideways: maximum position error
against the plane reaches 129 mm; one run stops at the finite patch boundary
after 460 steps. The other candidates complete 480 steps.

Post-release mechanical energy rises by at most 1.12e-16 J, i.e. rounding
scale. This is an important negative result: energy dissipation and collision
separation can both pass while the motion is wrong. Keeping only an energy
test would have accepted an unsuitable model.

## Status and next boundary

The force kernel and opt-in experiment remain available for research; there
is no editor switch or GPU implementation. Static negative controls now reject
promotion on positivity/continuity alone. The original successful moving and
held-release controls still pass. No new dress trajectory or performance
claim is made in this pass.

All 39 targeted CPU tests pass, including explicit negative tests that retain
the failed sliding/repulsion behavior as a promotion blocker. CPU and desktop
all-target Clippy, formatting and tracked-diff checks pass. A rebuilt static
probe produces identical JSON. The user's garment file hash is unchanged.

Before another whole-dress run, contact aggregation needs to reproduce
translation along a flat surface without depending on internal triangulation.
Investigate a consistent aggregate surface potential rather than independently
tuning these transition weights. The normalized local-field approach in
[Larionov et al.](https://elrnv.com/fcses_tog21.pdf), §4.1, is relevant to that
gate; their §7 explicitly excludes self-contact. It is not a drop-in answer
for folded cloth, and original OGC guards must remain in any experiment.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test two_sided_contact --example two_sided_contact_probe -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example two_sided_contact_probe -- --output output/two-sided-new/static.json
cargo run --locked --no-default-features -j 2 --example contact_patch_probe -- --two-sided-alpha 0.05,0.2,0.5,0.8 --output output/two-sided-new/pull.json
```

Raw evidence: `output/two-sided-contact-20261020`.
See [validation and hashes](two-sided-contact-validation.json).
