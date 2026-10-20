# Bounded residual search for contact reactions

CPU research follow-up to [unilateral contact transitions](unilateral-newton.md).
Production VBD/OGC, the editor and GPU code are unchanged.

## Question

At some failed pull/release iterations, a natural-residual Newton direction
requests a negative reaction from a contact whose current reaction is zero.
The nonnegative-cone safeguard correctly rejects it. Can a constrained
least-squares search find a different descent direction without attraction?

## Search model

`solver_lab::bounded_least_squares` minimizes a rectangular linear least-squares
residual with free leading variables and nonnegative trailing variables.
The latter are **new** contact reactions, not reaction increments. A working
set fixes some reactions at zero. QR subproblems solve for free variables;
a negative trial reaction moves the feasible iterate to the first encountered
bound. A negative reduced gradient releases a fixed variable. Column scaling
reduces unit-dependent conditioning. No normal equations, diagonal compliance,
poststep damping or physical energy clamp is added.

The report gives the original residual norm and the projected-gradient
optimality check in normalized coordinates. Finite precision and numerical
rank truncation can prevent convergence; the report or iteration-budget error
must not be treated as a successful physical timestep.

The contact adapter assembles the current natural-complementarity branch
`min(lambda / 1e-7, gap / 1e-10)` and the full discrete-force linearization.
Reaction branches remain residual rows instead of forced eliminations.
Positions are unconstrained in this linear least-squares model; geometric
feasibility is still enforced by the original OGC bounds, every swept
certificate and final physical gates. Consequently a good linear residual
alone cannot certify a useful collision-safe direction.

`--bounded-newton` requires `--unilateral-newton` and its prerequisites.
Without `--combined-newton`, it competes with the geometric-QP direction.
With `--combined-newton`, the unilateral direction is an additional candidate.
All proposals use the same nonlinear merit and collision checks.
`--fallback-bounded` additionally requires the combined mode and considers the
bounded candidate only when the ordinary QP/unilateral candidates find no
acceptable step. This tests whether it can rescue a failure without changing
earlier accepted directions. It currently still computes the unused bounded
proposal; performance is not the purpose of this oracle.

## Independent comparison

The official [SciPy `lsq_linear` documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.lsq_linear.html)
describes BVLS as a working-set method with unconstrained least-squares
subproblems. It was read as algorithmic context; this Rust implementation is
independent, supports only zero lower bounds on a trailing variable block,
and does not inherit SciPy's implementation or convergence guarantees.

`tests/fixtures/bounded-ls-oracle.json` records twelve seeded rectangular
problems and independent SciPy 1.15.3 BVLS solutions. The seed is 117083;
columns span eight orders of magnitude. Tests also cover bound entry/release,
inconsistent residuals, rank deficiency, zero columns and explicit failures.
This is a linear-kernel comparison, not yet a cloth or dress validation.
Regenerate with `python scripts/generate-bounded-ls-oracle.py`; the NumPy/SciPy
versions are recorded in the fixture.

The 79-variable linearization from eased held step 133 is also captured in
`tests/fixtures/bounded-contact-ls.json`. Rust and independently run SciPy BVLS
both give residual norm 12752.08189956, down from 43287.63558947 at the current
iterate. All proposed reactions are nonnegative. However the minimum predicted
geometric gap is -1.0243e-9 m, outside the unchanged 1e-10 m tolerance.
At the failed nonlinear iterate, swept certification returns `Unresolved` near the contact between
vertex 3 and face 8 in this synthetic fixture. `Unresolved` is not a collision
proof and remains a rejection.

This is a negative control: a converged reaction-bounded least-squares model
is not a feasible cloth step. `failed_nonlinear.line_search` now identifies
the rejected floor or primitive certificate. Bounded search captures the
current linearization, so a failed nonlinear iteration can be checked
independently. A subsequent constrained search must incorporate geometric
inequalities as hard constraints instead of spending clearance tolerance
to reduce its residual. No tolerance relaxation is justified here.

## Eight full protocols

Each run starts from the original placement and requests 180 timesteps on the
same 16-vertex sewn fixture, with all 266 VF/EE pairs and unchanged acceptance
thresholds. Full quality includes convergence, geometry and post-release energy.

| Search policy | Matching moving | Matching held | 10% eased moving | 10% eased held |
| --- | --- | --- | --- | --- |
| Bounded + geometric QP | **180, pass** | 133, fail | **180, pass** | **180, pass** |
| Bounded only as fallback to unilateral/QP | 103, fail | **180, pass** | **180, pass** | 133, fail |

The direct policy passes three of four protocols versus two for the previous
unilateral policy, but regresses matching-held release. There is still no single
policy that passes all four; selecting a different method after seeing each
case's result would not constitute a robust solver.

The three passing direct runs retain minimum separation 1 mm within roundoff,
maximum force residual below 9.79e-8 N, complementarity below 2.12e-13 J and
maximum post-release energy rise 1.061e-14 J. Patch displacement RMS during
the first ten steps after release is 8.52, 8.33 and 7.88 mm for matching moving,
eased moving and eased held, respectively. No sleep, pinning or added damping
is used. These measurements do not establish whole-dress perceptual fluidity.

The successful fallback trajectories reproduce the previous unilateral
positions and velocities exactly for all 180 steps. The fallback does not
rescue its failed complete protocols. It is a controlled negative result,
not a general improvement.

The isolated matching step 103 now converges in 28 iterations with force
residual 4.10e-8 N, contact work -1.020e-5 J and total-energy change -1.311e-5 J.
The isolated eased-held step 133 still fails as described above, despite the
direct full trajectory passing that protocol. Checkpoint success/failure must
not replace full-trajectory tests.

The passing direct CPU development samples cost 70.9–99.6 ms/step including
diagnostics; the fallback also computes unused proposals. These are dense
finite-difference oracles on 16 vertices, not production performance results.
Thirty-five targeted CPU tests and all-target CPU Clippy pass. Two legacy
endpoint-law checkpoints reproduce their exact positions and velocities.

[Validation and hashes](bounded-contact-search-validation.json) records all
eight runs, the independent failed-matrix oracle and checks. Reproduce with
`output/bounded-residual-20261020/reproduce.ps1`; the executable and source
archive are retained beside it. The editor, GPU path and user garment are
unchanged. Next work is a geometrically constrained residual search, preserving
these negative controls and the complete release matrix.
