# Discrete contact work: invariant force-row experiment

Follow-up: [a coupled timestep pilot](discrete-contact-solve.md) now tests these
force rows dynamically. Two isolated steps improve, but full sewn protocols
fail. The offline results below remain distinct from that integration attempt.

This is an offline force-row experiment, not a new cloth integrator. It
addresses the positive contact work measured in the
[sewn self-contact benchmark](sewn-self-contact.md). Production VBD/OGC, the
editor and the simulated benchmark trajectories remain unchanged.

## Literature actually consulted

- Hesch and Betsch, [3D NTS contact and conserving integration, 2011](https://www.ifm.kit.edu/download/cmnts.pdf):
  introduction and time-discretization section. Their mixed formulation couples
  a discrete gradient with configuration constraints and midpoint kinematics.
  It is a solid-contact formulation, not an implementation of cloth OGC.
- Hesch and Betsch, [energy-momentum mortar contact, 2009](https://www.ifm.kit.edu/download/ijnmemortarcontact.pdf):
  introduction and sections 4.1â€“4.2, especially equations 80â€“90. Quadratic
  invariants connect discrete work and momentum properties. The paper's mortar
  discretization is not implemented here.
- Gautam and Sauer, [adhesive contact time discretization, accepted 2012](https://www.sd.rub.de/downloads/publications/Gautam12_IJNMEp.pdf):
  abstract and introduction. Their warning that conserving energy does not
  establish convergence motivates retaining separate convergence gates.
  No adhesion model is implemented.

The code below is an independent adaptation of the invariant-gradient
construction to finite VF/EE distances. It is not a reproduction of those
complete algorithms.

## Construction

For four contact vertices, use the six invariants
`pi_ij = 0.5 |x_i - x_j|^2`. The finite-primitive distance is a function of
these invariants.

We minimize the VF/EE witness distance in the averaged endpoint metric. The
implementation represents it by pairs of old/new 3D vectors, with inner
product `(a_old.b_old + a_new.b_new)/2`. This avoids reconstructing coordinates
from a Gram matrix or adding diagonal regularization. Boundary features and
interior candidates are checked explicitly.

For signed barycentric weights `w` with `sum w = 0`, the squared witness
distance is `-2 sum(w_i w_j pi_ij)`. Thus the distance derivative at the
averaged invariants is `a_ij = -w_i w_j / d_mid`. The discrete derivative is

```
g_pi = a + ((d_new - d_old - a . delta_pi) / |delta_pi|^2) delta_pi
g_x  = D pi(x_mid)^T g_pi
```

The mapping back to positions uses equal/opposite central forces along
midpoint edges. In exact arithmetic this gives:

- `g_x . (x_new - x_old) = d_new - d_old`.
- Zero total internal reaction.
- Zero internal torque about midpoint positions.

These are properties of the row, not a proof of conservation for the current
backward-Euler integrator. Near invariant coincidence, a threshold based on
machine epsilon avoids dividing by a tiny squared difference; the actual
chain error and branch flag remain observable. Overflow/nonfinite inputs are
rejected. This is floating-point research code, not an exact certificate.

## Rejected shortcut: freeze the witness

A simpler norm secant can keep the endpoint barycentrics fixed across the
step. It has an appealing work identity for that *fixed* witness, but this is
not the actual closest distance when the witness slides.

The negative control moves a point 0.4 m along a plane at a constant 1 mm
gap. The frozen-witness row develops a tangential component exceeding 0.9 in
magnitude per unit multiplier. It would act as artificial friction. The
invariant row has no measured tangential component above the 1e-10 test
tolerance. A corresponding EE sliding test also rejects the shortcut.

The invariant row is not necessarily a unit normal. A large sliding increment
can substantially change its magnitude; multiplier scaling and nonlinear
conditioning still require investigation.

## Audit on the two recorded energy-increasing steps

The synthetic fixture is shared in `examples/support/deforming_fixture.rs`.
`--capture-reactions` records the physical multipliers after convergence.
Both resumed steps reproduce the saved positions **and velocities exactly**.
The offline audit first reconstructs the original contact work, including the
analytic floor, then substitutes force rows at those same positions and
multipliers.

| Captured step | Original contact work | Invariant-row work | Frozen-witness work |
| --- | ---: | ---: | ---: |
| Matching seam, moving release, 176 | +1.62948e-6 J | -5.84510e-7 J | -1.39271e-6 J |
| 10% ease, moving release, 163 | +5.79017e-8 J | +3.77e-20 J | -2.75680e-6 J |

The first invariant result includes -5.21525e-7 J of unchanged floor work.
The tiny positive value in the second row is numerical roundoff.
Across these eight captured VF/EE reactions:

- Maximum distance-chain error: 1.42e-16 m.
- Maximum resultant per unit multiplier: 1.78e-15.
- Maximum midpoint torque per unit multiplier: 4.63e-17 m.

See [the machine-readable audit](discrete-contact-validation.json).
**This does not predict a corrected trajectory or its energy.** Multipliers
would change when solving with a different force row. The full moving-release
energy failures from the preceding experiment remain unresolved.

## Validation and integration boundary

Six kernel tests cover seeded finite-primitive comparisons, the distance chain
rule, resultant/torque, rigid transformations and scale changes, reversal,
static and shrinking-motion limits, flat sliding, tiny increments and invalid
inputs. A tunneling counterexample satisfies the work identity but fails the
existing swept collision certificate: the work construction must never be
used to bypass geometric checks.

The complete targeted suite contains 52 CPU tests. No GPU or new whole-dress
simulation is involved.

Do **not** substitute this row for `Row.jacobian` in the current contact QP.
That QP uses the same geometric derivative for contact feasibility and
reaction forces. The discrete force row generally differs from the derivative
of the endpoint gap. A consistent integration experiment must distinguish
those two roles and solve the resulting coupled equations.

Next steps:

1. Implement a separate nonlinear contact solve with endpoint feasibility,
   nonnegative multipliers, discrete force rows and full residual checks.
2. Retain OGC bounds and swept certificates. Evaluate work, contact-normal
   consistency, conditioning and timestep convergence independently.
3. Treat material work consistently too: a nonconvex material remainder can
   still increase energy even when contact work is nonpositive.
4. Run full moving/held release, folds and eased seams before evaluating a
   sparse/GPU implementation or replaying the dress.

## Reproduction

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test discrete_contact -- --test-threads=2
cargo build --locked --no-default-features -j 2 --example deforming_contact_probe --example discrete_contact_probe
```

Generate the matching/eased full traces with the preceding benchmark, then:

```powershell
target/debug/examples/deforming_contact_probe.exe --independent-block --pivot-block --swept-repair --accumulate-repair --iterations 384 --sewn-ease 0 --audit-energy --capture-reactions --resume output/sewn-self-contact-20261020/pivot-ease0-moving.json --resume-after 175 --steps 177 --output output/discrete-contact-20261020/matching-capture.json
target/debug/examples/discrete_contact_probe.exe --input output/discrete-contact-20261020/matching-capture.json --output output/discrete-contact-20261020/matching-work.json
```

For the eased case use `--sewn-ease 0.1`, its matching trace,
`--resume-after 162 --steps 164`. The offline tool never advances time or
accepts a new collision state.
