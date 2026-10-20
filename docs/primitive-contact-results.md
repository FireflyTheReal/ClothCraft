# Finite-primitive experiment: results

**Decision: not promoted.** Persistent finite distances remove the measured
refresh discontinuities, but summing their contributions introduces artificial
tangential forces. The existing VBD/OGC model remains the editor default.
The [implementation and coverage argument](primitive-contact.md) describe
exactly what this CPU ablation changes.

## Refresh discontinuities

The public EE and concave-PF fixtures, plus the captured EE and PF transitions
from the dress, each use 65 interpolated configurations. In all four probes,
the largest absolute candidate energy change caused solely by rebuilding
contacts is **0 J at the recorded precision**. Every sample has exactly the
same OGC radii, directional regions and minimum distance as the control.
The separate pair-coverage test exercises activation outside the original
query radius but inside the enlarged force halo.

On a fresh six-step replay of the dress, with 16 VBD iterations and `dt=1/60`:

| Metric | Existing law | Finite primitives |
| --- | ---: | ---: |
| Contact refreshes | 9 | 9 |
| Minimum refresh delta, J | −0.447956452 | 0 |
| Maximum refresh delta, J | +0.088282421 | 0 |
| Sum of absolute refresh deltas, J | 0.729492659 | 0 |
| Accepted steps | 6 | 6 |
| Endpoint intersection audits | Pass | Pass |

The control's final complete state is byte-identical to the preceding
production-capture replay. Its physical behavior has not changed.
These six steps are not a settling or performance benchmark: detailed refresh
capture adds work, and a 0.1-second interval does not establish equilibrium.
Contact energies and force residuals across different laws are not
equal-objective comparisons.

## Planar sliding falsifies the candidate

A point moves over two coplanar triangles at a constant 1 mm gap, with no
friction and the existing 3 mm contact radius. The test samples 161 positions
over 8 mm, crossing their common diagonal.

| Metric | Existing law | Finite primitives |
| --- | ---: | ---: |
| Minimum contact energy, J | 0.203729649 | 0.203729649 |
| Maximum contact energy, J | 0.203729649 | 0.407459299 |
| Maximum artificial tangential force, N | 0 | 79.545455 |

The candidate's energy is continuous, but its gradient resists motion on a
flat frictionless surface. This is precisely the kind of sticking the user
rejected. Lower jitter alone cannot justify enabling it. The single-triangle
sliding/pull/release test still matches every production state exactly;
that test cannot detect aggregate forces across a triangulation seam.

## Matched dress pull/release

Four sequential CPU runs start from the same complete saved state, use
16 iterations at 1/60 s, and run for 40 steps (2/3 s). Each contact law has
a quiet control and a 40 mm grab trajectory: five quiet steps, ten pull,
five hold and twenty release. No damping or sleep parameter is changed.
All 160 accepted endpoint states pass the intersection diagnostic.

| Run | Mean full step, ms | P95, ms | Final seam gap, mm | P95 extension, % |
| --- | ---: | ---: | ---: | ---: |
| Existing, quiet | 946.07 | 1,053.59 | 17.66 | 22.30 |
| Existing, pull/release | 958.19 | 1,098.11 | 16.60 | 22.20 |
| Primitive, quiet | 1,389.67 | 1,637.34 | 20.03 | 20.49 |
| Primitive, pull/release | 1,426.66 | 1,739.51 | 21.54 | 20.29 |

Relative to its quiet control, peak grab displacement is 23.12 mm for the
existing law and 27.03 mm for the candidate. Net movement after release is
24.38 and 27.39 mm respectively. These measurements establish that the
candidate did not simply freeze the grabbed region; they do not certify
correct sliding or an equilibrated drape. Minimum audited separation is
0.658 mm for the control and 2.073 mm for the candidate.

The candidate retains 79,883 contact terms at the final quiet
state versus 14,209 in the control and costs about 47–49% more in these runs.
Many extra terms are inactive but still participate in incidence and coloring.
Timings exclude export and offline audits, discard ten warm-up steps and are
single-run CPU measurements. They are not GPU frame times or 60-step/s claims.
No steady-state jitter conclusion is drawn from this short startup interval.

A compatibility replay with the prior executable and another replay with the
new executable produce **exactly the same final state** as the first current
control, including all positions and velocities. Their mean times are
976.35 and 1,436.56 ms, versus 946.07 ms for the first current control;
the repeated control's P95 rises to 2,865.94 ms. This large timing variation
prevents isolating a reliable default-path performance change or treating the
47–49% candidate overhead as a stable ratio. The per-run timings are retained
as observations, not a speedup/regression guarantee. The force-law rejection
does not depend on those noisy timings.

## Evidence

Local, ignored directory: `output/primitive-contact-20261020`.
The four transition probes are `synthetic-ee.json`, `synthetic-pf.json`,
`dress-ee.json` and `dress-pf.json`; planar forces are in `slide.json`.
The short diagnostic runs are `vbd-control` and `vbd-primitive`.
Private garment geometry is not embedded in tests or physics code.

The next contact-law experiment needs a coherent aggregation/directional
model, not simply more damping or a different optimizer over this candidate.
The additional planar-slide gate should remain part of solver comparisons.

Final validation: **183 CPU tests pass**, including 442 finite-distance
samples and 1,640 shortcut/full-AD vertex blocks. All-target CPU and desktop
Clippy pass with warnings denied, and formatting passes. GPU tests were not
run. The original saved garment hash is unchanged. The machine-readable
[validation summary](primitive-contact-validation.json) explicitly marks the
planar sliding promotion gate as failed and timing repeatability as
insufficient for a reliable speed comparison.

`scripts/summarize-primitive-contact.py output/primitive-contact-20261020
--plot output/primitive-contact-20261020/planar-slide.png` reproduces the report
and the static force/energy plot from the recorded evidence.
