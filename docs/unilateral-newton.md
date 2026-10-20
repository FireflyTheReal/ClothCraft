# Contact activation and release in the discrete-force pilot

This CPU research experiment follows [the rank-deficient search](contact-rank.md).
It is not an editor backend or a whole-dress validation.

## Linearized complementarity problem

At the current positions and nonnegative reaction history, let A be the derivative
of the full discrete-force residual with respect to position. Let f be the
material/inertia/grab gradient, F the discrete reaction columns, and G the ordinary
geometric Jacobian. The trial equations use **new** multipliers, not increments:

```
A dx + f - F lambda_new = 0
G dx - offset >= 0
lambda_new >= 0
lambda_new * (G dx - offset) = 0
```

A includes derivatives of all current reactions. Inactive target reactions are
not prematurely removed from this derivative. The geometric-QP solution only
supplies an initial working-set guess. A rank-aware equality solve proposes a
step; a negative reaction releases that equality, and a violated inactive gap
activates its equality. All geometric rows remain in the residual and feasibility
checks. No negative reaction is accepted. Repeated working sets and pivot-budget
exhaustion are explicit errors. Inconsistent equality solves are reported with
their full residual, not marked solved.

`--unilateral-newton` requires `--rank-newton --newton-forces --discrete-forces`.
As before, a proposal is not a physical timestep: it must pass residual merit,
OGC bounds, every swept VF/EE certificate and the final physical convergence gates.
No tolerance, damping, material rest or energy clamp is changed. The new module
`solver_lab::unilateral_newton` is a small dense search oracle, not a scalable
cloth implementation.

## Research context actually read

The [Hintermueller-Ito-Kunisch publisher abstract](https://epubs.siam.org/doi/10.1137/S1052623401383558)
connects particular primal-dual active-set strategies with semismooth Newton
methods. Only the public abstract was read here; the full article is access-gated.
The implementation above is an independent single-pivot trial, not a reproduction
of that paper or a transfer of its convergence guarantees to nonmonotone cloth.

The [Papadopoulos-Hintermueller 2026 abstract](https://arxiv.org/abs/2607.26622v2)
reports deterioration of PDAS iteration counts under mesh refinement for studied
obstacle/Signorini problems. This supports an explicit mesh-scaling gate for
future work; the abstract does not establish a complexity bound for this solver.

## Tests

Kernel tests exercise activation, release without attraction, distinct force and
geometry rows, redundant contacts, inconsistent equalities, a nonmonotone cycle
and budget exhaustion. Full synthetic pull/release tests remain necessary; a
passing linear system alone cannot establish cloth robustness or fluidity.

## Replay diagnosis and natural-residual direction

The first unilateral replays still reject matching step 87 (force residual
9.906e-5 N) and eased held step 133 (3.408e-3 N). At both failed iterations,
all 32 backtracking trials of each available direction satisfy the unchanged
geometric gates, but none decreases the actual natural-complementarity merit.
Thus collision rejection is not the immediate cause of these two failures.
The full trial trace is now retained in `failed_nonlinear.line_search` when
`--audit-iterations` is enabled. An earlier explanation based solely on negative
proposed reactions is insufficient for these unilateral replays.

The optional `--natural-newton` experiment chooses each linearized branch from
the **current** residual `min(lambda / 1e-7, gap / 1e-10)`. A reaction branch
sets the proposed multiplier to zero; a gap branch linearizes its geometric
equality. The full current reaction derivative remains in the force block.
This differs from choosing the equality set of a future LCP solution: that
choice need not be a descent direction for the current merit.

Proposed negative reactions are not accepted. The line search restricts its
length to 99% of the first reaction-cone boundary; a direction pointing out
of the cone from zero is rejected. This is a search safeguard, not a change
to physical friction, compliance or convergence tolerances. It can itself
stall at the cone boundary, and the finite-difference derivative can cross
feature transitions. There is no convergence guarantee. The production
solver is unchanged. A scalar directional-derivative test covers both
natural-residual branches; complete trajectory results must be inspected
separately.

`--combined-newton` compares the unilateral and natural directions alongside
the geometric-QP direction, using the same nonlinear merit and collision
checks for every candidate. It conflicts with `--natural-newton`, which tests
the natural direction alone alongside the QP. Lower local merit is not a
guarantee of better global trajectory convergence; retain the separate controls.

## Full trajectory results

Twelve protocols were run from the original synthetic rest/placement, with
180 requested steps each, 16 vertices, one seam and 266 eligible VF/EE pairs.
Numbers below are **converged** steps, excluding a recorded exhausted-budget
frame. A pass requires the original force, complementarity, geometry, swept
acceptance and post-release energy gates, not just reaching the last step.

| Direction experiment | Matching moving | Matching held | 10% eased moving | 10% eased held |
| --- | --- | --- | --- | --- |
| Unilateral | 103, fail | **180, pass** | **180, pass** | 133, fail |
| Natural residual | 62, fail | 62, fail | **180, pass** | 133, fail |
| Both candidates | 103, fail | 133, fail | **180, pass** | 133, fail |

The unilateral variant improves the previous rank-search matrix from one to
two passing protocols. Both passing unilateral runs have no measured positive
post-release energy increment, force norm below 9.96e-8 N, complementarity below
1.78e-13 J and minimum distance 1 mm within roundoff. Patch displacement RMS
over the first ten steps after release is 7.92 mm (matching held) and 8.28 mm
(eased moving); low motion is not used as a substitute for force convergence.
These are not whole-dress or perceptual-fluidity results.

Natural-residual search solves the **isolated** matching step 87 in 12 iterations:
force residual 3.85e-9 N, complementarity 1.19e-14 J, separation
0.000999999999797 m. The positive total-energy change occurs during the external
pull; the contact's work is negative. Nevertheless the same method started
from the beginning fails earlier, at step 62. An isolated replay is therefore
not a replacement for full-trajectory validation.

At eased held step 133, the natural proposal is blocked by the nonnegative
reaction cone. The combined search reduces the residual but exhausts its
384-iteration budget. It also regresses the passing matching-held unilateral
run. Choosing the smallest instantaneous merit does not guarantee a better
sequence of physical timesteps. None of these candidates is promoted.

The two passing unilateral samples cost 75.3 and 57.0 ms/step, respectively,
in CPU development builds including diagnostics. These are small dense
finite-difference oracles, not realtime results or a production speedup claim.

Evidence, both executable versions, source archives and hashes are under
`output/unilateral-newton-20261020`. `reproduce.ps1` verifies the selected
executable hash before running all twelve protocols. It preserves failure
states and uses `scripts/summarize-deforming-contact.py` for the full gates.
Do not interpret an exhausted-budget diagnostic frame as a converged step.

Next: a constrained residual-minimization direction at the reaction-cone
boundary, with a verified directional derivative across contact-feature
transitions. Preserve the unilateral controls and the full pull/hold/release
matrix. No scene-specific contact suppression, added damping or relaxed
collision tolerances is justified by these results.
