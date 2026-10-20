# Moving separating planes for the CPU contact oracle

Independent research construction following the
[long-tail failure](contact-long-tail.md). This is not a production OGC change
or a claim to reproduce a published cloth backend.

At the last accepted search iterations of the failed eased-moving step 479,
the OGC prebound is approximately one, but the swept test rejects most larger
trials as `Unresolved`. This motivates a stronger certificate, not treating
uncertainty as safety or increasing clearance tolerance.

## Continuous separation argument

Within a time interval, each primitive vertex moves affinely. Choose endpoint
separating directions `n0`, `n1` and interpolate `n(u)=(1-u)n0+u*n1`. These
directions do not have to remain closest-point normals.

For every vertex pair, one from each convex primitive, define endpoint
differences `r0`, `r1`. Its projected separation is a quadratic polynomial,
with Bernstein coefficients

```
c0 = dot(n0, r0)
c1 = (dot(n0, r1) + dot(n1, r0)) / 2
c2 = dot(n1, r1)
```

Bernstein weights on `[0,1]` are nonnegative and sum to one, so the smallest
coefficient bounds this projection throughout the interval. Convexity of the
norm gives `|n(u)| <= M = max(|n0|, |n1|)`. If every coefficient for every vertex
pair is at least `delta*M`, all convex combinations on the two primitives have
projected separation at least `delta*|n(u)|`. Cauchy–Schwarz then gives distance
at least `delta`. A zero interpolated normal cannot satisfy the positive bound.

This checks the entire linearly interpolated path, not just its endpoints.
Rotating supports can be certified more tightly than with a fixed plane,
without assuming that the actual primitive distance is linear or monotone.

## Implementation and limits

`solver_lab::deforming_contact::certify_moving_planes` first executes the
existing fixed-plane/sample checks. Only when these cannot certify an interval
does it try the moving plane. Failure still subdivides; depth and interval
budgets are unchanged. An exhausted budget remains `Unresolved`, never `Safe`.
The old `certify` API and default policy retain their original path.

The f64 implementation subtracts `256*epsilon*coordinate_magnitude` from
each coefficient and adds `16*epsilon` to the endpoint norm bound. This is
conservative floating-point engineering, not a formally verified
interval-arithmetic implementation for arbitrary extreme coordinates.
The separation target is the existing `clearance - tolerance`.

The opt-in probe flag is `--moving-plane-certificates`. It applies to candidate
generation, path repair and final swept acceptance consistently; it does not
remove pairs, alter material rest, change physical reactions, or bypass OGC
clipping. New trajectories and energy/convergence must still be validated.

Tests include co-rotating VF/EE primitives with only a small clearance margin,
safe endpoints whose midpoint loses clearance, and true tunneling. A successful
kernel test alone does not establish a robust nonlinear cloth solver.

## Final isolated replay, 20 October 2026

All six `deforming_contact` CPU tests pass, including the two new tests above.
From the exact accepted frame 478 of the failed eased-moving tail, the same
384-iteration budget with `--moving-plane-certificates` converges in eight
iterations. Exact residual-sample reuse is also enabled; its separate cached
versus uncached controls preserve every pre-existing diagnostic field.

| Measure | Fixed-plane failed step | Moving-plane replay |
| --- | ---: | ---: |
| Nonlinear iterations | 384 (exhausted) | 8 |
| Force residual (N) | 8.29067e-5 | 2.69542e-9 |
| Complementarity (J) | 8.46104e-8 | 5.31585e-15 |
| Minimum pair separation (m) | 0.0009999999879104751 | 0.000999999999462732 |

The replay preserves the floor clearance of 0.001 m. Its mechanical energy
change is -4.35488e-8 J, contact work -2.65505e-8 J and balance closure error
1.225e-20 J. All final physical gates pass with the existing thresholds.
Measured probe elapsed time is 1.723 s on this one 16-vertex CPU step; it is
not a repeated performance comparison or a realtime cloth result.

Raw replay, source/binary hashes, reproduction script and gate evaluation are
under `output/moving-separator-20261020`; the public compact result is
[validation](moving-contact-separator-validation.json).
No long trajectory, whole-dress replay or GPU run was performed with this
option. Earlier short pull/release protocols belong to the preceding
fixed-plane policy, not this final option. The editor and production solver
remain unchanged. Work is paused after this isolated check at the owner's
request; do not interpret it as a general jitter fix.
