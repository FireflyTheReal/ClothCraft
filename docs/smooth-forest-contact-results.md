# Smooth forest contact: kernel results

Historical first pass. The subsequent [factorization and endpoint-boundary
pass](forest-factor-and-boundary.md) fixes an additional padding-dependent
Hessian and changes the dense limit to 128 nodes per structural component.

20 October 2026. **Research kernel only.** The editor still uses its existing
VBD/OGC contact law. No production forces, OGC bounds, GPU kernels, damping,
prediction, or saved garment were changed. This is not a measured dress jitter
fix, a new editor backend, or a speed comparison.

## What changed

The previous hard forest avoids negative wire densities but makes a discrete
choice of contact links. A symmetric target triangle exposes a 222.5 N jump in
its force. `src/ogc/smooth_forest.rs` replaces that choice with differentiated
rooted-forest inclusion probabilities. The construction, exact-arithmetic
barrier bound and implementation are in the [derivation](smooth-forest-contact-proposal.md).

The new kernel passes tiny-graph enumeration and all-coordinate derivative,
objectivity and force/torque tests. It removes the measured hard-forest cusp:
at `tau = 1e-5 J`, the force difference between source positions `x = ±h` is
approximately 0.00326, 0.000326 and 0.0000326 N for `h = 1e-7, 1e-8, 1e-9 m`.
These are finite-separation force differences tending to zero, not a remaining
finite force discontinuity. The geometry stays strictly separated.

An exact shared-endpoint test also caught an extra `-225000 N/m` tangential
Hessian entry in the naive two-node implementation. Algebraic common-feature
cancellation before differentiation removes it. The combined energy, gradient
and Hessian now agree with the analytical straight-line expression, including
the exact endpoint. Higher-valence feature ties still need their own treatment.

## Sliding bias is real

The smooth model leaves a residual `c*tau/(tau+2*c)` on a two-edge patch.
Its gradient is an artificial tangential force, even though it is conservative.
The static scan samples logarithmically near activation to resolve the narrow
force layer that a coarse scan or 60 Hz trajectory could miss.

| `tau` (J) | Peak tangential force (N) | Maximum energy excess over the plane (J) |
| --- | ---: | ---: |
| 1e-3 | 3.0531 | 4.98776e-4 |
| 1e-5 | 0.30601 | 4.99988e-6 |
| 1e-7 | 0.030608 | 4.999999e-8 |
| 1e-9 | 0.0030608 | 5.00000e-10 |
| 1e-11 | 0.00030609 | 5.00000e-12 |

The normal repulsion remains at least 225 N on this planar scan, within
roundoff. Reducing `tau` reduces the bias but does not make the model exactly
frictionless. It is a dimensioned model parameter, not a convergence tolerance.

## Pull, hold and release

`contact_patch_probe` runs the protocol in [contact-patch.md](contact-patch.md):
20 g rigid probe, 225 N normal preload, 8 mm handle motion, 480 steps at 1/60 s.
It releases once while moving and once after holding. No damping, friction,
sleep or velocity correction. The diagnostic Newton integrator uses the
existing OGC truncation against the fixed plane; this is not a deforming cloth
collision test or a call to `VbdSolver::step`.

All ten smooth-model runs complete 480 steps, remain separated, have zero
unfinished solves, and show no measured post-release mechanical-energy rise.
The following maxima cover both protocols and every recorded endpoint:

| `tau` (J) | Position error vs analytical plane (m) | Velocity error (m/s) |
| --- | ---: | ---: |
| 1e-3 | 5.80249e-4 | 3.38631e-2 |
| 1e-5 | 2.67576e-5 | 1.60543e-3 |
| 1e-7 | 9.63815e-10 | 5.78117e-8 |
| 1e-9 | 7.76379e-13 | 4.61000e-11 |
| 1e-11 | 7.76316e-17 | 4.60586e-15 |

The `1e-7` and `1e-9 J` candidates pass the automated moving/held-release
trajectory gate. Moving release retains approximately 4 mm/s horizontal speed.
Tiny endpoint errors do **not** cancel the static force-layer evidence above.
The two largest `tau` values cause noticeable velocity spikes during dragging;
they are retained as negative controls, not recommended settings.

## Dense graphs and dress snapshots

The earlier tetrahedral wire counterexample attracted the source with roughly
446.6 N under the signed sum. The smooth forest repels it with about 228.5 N
at the original 1 mm height. A six-height sweep stays repulsive or inactive for
all five `tau` values. This tests one dense geometry; positivity alone does not
prove correct force direction for every arrangement.

The conditioning probe checks 4/8/16/32-node graphs, disconnected strong pairs
joined by explicit zero links, two node permutations, and `tau` from 1e-3 to
1e-16 J. All 48 evaluations complete. Maximum energy error against the
independent two-node formula is 3.56e-15 J; maximum probability error is
5.56e-16. This is not a guarantee for all matrices or automatic derivatives
at extremely small `tau`.

Selected graphs were evaluated at the frozen dress checkpoint (49.45 s) and a
previously recorded production-control pull/release endpoint (50.1167 s).
The audit selects both the 32 lowest signed densities and the 32 largest
active target graphs, deduplicating source edges per radius.

| Radius | Sources sampled, checkpoint / release | Largest target graph, checkpoint / release | Failed evaluations, checkpoint / release |
| --- | ---: | ---: | ---: |
| 3 mm | 64 / 64 | 6 / 6 | 0 / 0 |
| 6 mm | 59 / 58 | 85 / 78 | 0 / 0 |
| 12 mm | 48 / 46 | 159 / 147 | 35 / 40 |

Each source has five `tau` evaluations. The 75 failures at 12 mm explicitly
exceed this dense prototype's 128-node limit; no nodes are silently dropped.
All 1,620 completed evaluations satisfy the largest-primitive energy bound
within measured precision. These are **selected snapshot values**, not forces
or steps of the robe under this model. No new dress trajectory was generated.

## Promotion gates still open

1. A compatible vertex–face model and near-parallel edge treatment.
2. Exact feature-tie derivatives for general target graphs.
3. Complete high-arity incidence: all target vertices affect the energy,
   gradient, Hessian, coloring and objective checks. Freezing probabilities
   or fitting this into an unrelated four-vertex term would be inconsistent.
4. Conservative force halos and continuity when graph membership changes.
   Original geometric collision guards and seam exclusions must remain.
5. Full dress replay, independent collision audits, finite-iteration residuals
   and matched pull/release; only then complete-step cost measurements.

Local evidence is under `output/smooth-forest-20261020`. Public tests are
`tests/smooth_forest.rs` and the regression tests in `contact_patch_probe`.
The pass validates 26 distinct targeted CPU tests; CPU and desktop all-target
Clippy, formatting and tracked-diff whitespace checks pass. The final rebuilt
probe executables reproduce the preceding JSON results exactly.
The [machine-readable record](smooth-forest-contact-validation.json) records
the actual checks and source/evidence hashes.
