# Geometrically constrained residual search

CPU research follow-up to [reaction-bounded search](bounded-contact-search.md).
Production VBD/OGC, editor behavior, material rest and GPU kernels are unchanged.

Follow-up: [long release tails](contact-long-tail.md) expose later nonlinear
failures despite the four short passes below. Those short results do not
establish durable settling.

## Model and safeguards

The previous reaction-bounded least-squares optimum requested a geometric
penetration of roughly 1 nm in the captured synthetic case. Instead of loosening
the swept gate, this experiment adds every current linearized geometric row
to the search problem as an inequality, alongside nonnegative new reactions:

```
minimize || A z - b ||²
subject to G dx >= offset, lambda_new >= 0
z = [dx, lambda_new]
```

`A z-b` remains the scaled linearization of the full discrete-force and
natural-complementarity residual. Its construction is shared with the previous
bounded method. These are search equations; actual nonlinear force balance,
complementarity and swept geometry still decide physical acceptance.

`solver_lab::constrained_least_squares` starts with a feasible-point projection
using the existing small convex QP oracle. A primal working set then minimizes
the residual subject to active equalities. Twice-reorthogonalized row bases
construct a particular solution and null space; the reduced least-squares
problem uses the existing rank-aware QR oracle. Steps stop at the first
inactive inequality. Negative active multipliers release constraints.
No normal equations, physical compliance, added damping or energy clamp is used.
Working-set multipliers are dual variables of the search problem; they are
distinct from the physical contact reactions stored inside `z`.

Rank thresholds, equality residual checks, feasible-start failure and iteration
limits are explicit. The report's normalized feasibility and scaled stationarity
are numerical search diagnostics, not physical force/energy convergence.
Near-dependent rows and cycling remain possible limitations. All original
inequalities remain in feasibility checks.

The contact adapter rejects a reaction proposal below -1e-11 N and clips only
smaller negative numerical bound slack to zero in its **search multipliers**.
It then recomputes the actual nonlinear force and merit. Positions and velocities
are not clipped by this operation. The unchanged OGC and swept certificates
still protect geometry; hard *linearized* constraints alone do not certify it.

`--constrained-newton` requires `--bounded-newton` and its prerequisites.
It can also be tested with `--combined-newton --fallback-bounded`; those flags
retain their prior candidate-selection meaning.

## Independent checks

The [LAPACK constrained least-squares documentation](https://www.netlib.org/lapack/explore-html/db/de8/group__gglse_gaa610b7fa3d5cd43cbcc2d7f44b2ce8b4.html)
was consulted for the equality-constrained formulation and rank assumptions.
LAPACK's driver uses generalized RQ factorization. This implementation instead
uses an independently written null-space construction and the local QR kernel;
it is not a LAPACK port or a claim of equivalent numerical robustness.

For the captured 79-variable matrix, independently run SciPy 1.15.3 SLSQP
finds residual norm 12752.09268679964, versus 12752.09268679968 in Rust.
The Rust normalized feasibility violation is 1.30e-26 and its working set
takes 25 iterations. The independent reference is stored in
`tests/fixtures/constrained-contact-ls-oracle.json`; input remains the public
synthetic fixture `bounded-contact-ls.json`, not private garment data.

Tests cover manufactured convex KKT optima with known active multipliers,
entry/release, duplicate constraints, infeasible starts and the captured
contact matrix. An isolated linear optimum does not validate a cloth trajectory.
`examples/constrained_ls_replay.rs` replays the matrix without running cloth.
`scripts/generate-constrained-ls-oracles.py` regenerates the manufactured
KKT fixtures and independent SLSQP reference; NumPy/SciPy are offline tools only.

## Full protocol matrix

The same direct constrained policy completes all four 180-step protocols from
original placement, on the 16-vertex fixture with one seam and 266 VF/EE pairs.
There is no per-case solver selection or tuning of acceptance thresholds.

| Seam lengths | Release | Converged steps | Full quality gate |
| --- | --- | ---: | --- |
| Matching | Moving | 180 | Pass |
| Matching | Held | 180 | Pass |
| 10% eased | Moving | 180 | Pass |
| 10% eased | Held | 180 | Pass |

Across these runs, maximum force norm is 9.95e-8 N, maximum complementarity
5.23e-13 J and maximum post-release energy increase 2.004e-14 J. Minimum
separation remains 1 mm within roundoff. The geometric thresholds, OGC bounds
and every swept certificate are unchanged. Whole-patch displacement RMS over
the first ten post-release steps ranges from 7.89 to 8.44 mm; success is not
obtained by freezing the fixture.

Both previously failed held checkpoints also complete the isolated step 133.
Matching takes 11 nonlinear iterations, with force residual 1.80e-12 N,
contact work -9.79e-6 J and total-energy change -1.372e-5 J. Eased takes 53
iterations, with force residual 7.76e-9 N, contact work -1.032e-5 J and
total-energy change -1.529e-5 J. These are additional controls, not substitutes
for the full protocols.

Thirty-nine targeted CPU tests and all-target CPU Clippy pass. Two endpoint-law
checkpoint replays and the previous bounded-search step 103 reproduce exact
positions and velocities. New references include twelve manufactured KKT
problems plus the independent constrained SLSQP result. No GPU was used.

**Scope limit:** this establishes four short synthetic release protocols,
not long-term immobility, arbitrary-cloth robustness, perceptual fluidity or
the user's full dress. The dense finite-difference implementation costs
311–513 ms/step in these single CPU development samples, including diagnostics
and uncontrolled background workstation load. It is not a realtime backend
or a production performance comparison. The editor remains on its existing
solver.

Evidence, source archive, both executables and reproduction commands are in
`output/constrained-residual-20261020`; [validation](constrained-contact-search-validation.json)
retains hashes, all four traces and motion/energy gates.

Next gates are longer quiet/release tails, varied fold angles and easing,
time-step/mesh refinement and frictionless sliding. Then replace expensive
dense finite-difference search with a sparse implementation, validating each
optimization against this reference before whole-dress comparison. Four passes
do not close the solver research objective.
