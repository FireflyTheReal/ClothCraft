# EE transition experiment: results

**Decision: keep the experiment disabled.** It removes the diagnosed EE
activation discontinuity and preserves the tested dress's pull/release
response, but does not uniformly reduce residual motion and adds CPU cost.
The current editor/GPU solver is unchanged. Read [the method and limitations](edge-transition.md) and
[the machine-readable measurements](edge-transition-validation.json).

## Transition evidence

The public eight-vertex fixture produces a 0.565853 J EE jump under the
original law. At the first sampled point inside the edge, the experimental
energy is 6.40e−15 J and the gradient norm is 7.52e−7 N, versus 1,378 N for the
original term. All 65 sampled configurations are separated; both models have
identical displacement radii and directional half-spaces.

The original dress event was captured in the **XPBD-primal research solve**.
Over the first six sweeps of that one-step reproduction, the maximum absolute
refresh jump changes from 0.619052638 J to 2.365757e−7 J (the remaining event
is PF). Its final fresh residual changes from 7.766 N to 0.865 N. These are
different experimental trajectories after the force-law change, not proof
that all jitter in production VBD comes from that one pair. The separate
fixed-geometry probe addresses the actual EE transition itself.

At the captured geometry, the new EE energy is 3.4624e−8 J and its gradient
norm is 0.02459 N, versus 0.61905 J and 1,798.95 N for the old term. The pair
is retained on both sides of the boundary. Its own refresh contribution
therefore vanishes; the remaining fixed-geometry total is −1.8363e−7 J (PF).
Both neighbors rejecting the original endpoint test belong to the opposing
edge. This is a local folded-surface configuration, not an avatar-only issue.

**Crucial production control:** the passive audit of the first six actual
VBD steps records nine internal refreshes. The largest positive jump is
0.088282 J with the original model and 0.088403 J with the experiment.
The positive sums are 0.088282 J and 0.099798 J respectively. Thus removing
the identified EE event does not remove contact-set energy changes from VBD.
The remaining event's primitive family has not yet been classified; the
total-energy diagnostic alone cannot identify it.

## Existing VBD solver on the saved dress

Four CPU runs start from the same 5,126-vertex saved state and immutable
material rest configuration. Each takes 120 steps at 1/60 s with 16 local
sweeps, unchanged damping, gravity, materials and seams. The pull moves the
same initial attachment target by 40 mm over 0.5 s, holds for 0.25 s, then
releases for 0.75 s. Each pulled run has an otherwise identical quiet control.

All **480 accepted endpoints pass** the surface intersection diagnostic under
the existing local seam exclusions. Minimum observed separation stays above
0.657 mm in these runs; this is not an exact geometric certificate.

| Final quantity | Original quiet | Experiment quiet | Original pull/release | Experiment pull/release |
|---|---:|---:|---:|---:|
| Maximum sewing gap, mm | 8.54 | 8.28 | 8.00 | 7.98 |
| Extension P95, % | 5.20 | 4.92 | 5.66 | 5.59 |

Peak attachment response relative to each quiet control is **35.34 mm vs
35.58 mm**. Net movement after release is **37.32 mm vs 37.84 mm**. This checks
that the candidate can move under load and after release; it does not prove
equivalent drapes or calibrated material behavior. The public supported-patch
test additionally checks sliding, lifting and falling, with exactly identical
baseline/experimental trajectories when no EE interior contribution occurs.

The final 30-step highpass RMS in quiet runs is mixed:

| Panel | Original, µm | Experiment, µm |
|---|---:|---:|
| front | 49.75 | 49.28 |
| back | 47.69 | 50.11 |
| panel_1 | 70.05 | 59.19 |
| panel_2 | 93.73 | 76.00 |
| panel_3 | 113.02 | 105.84 |

The disconnected falling panel is excluded from this table, but retained in
the raw metrics. The dress is still evolving: these numbers include draping
acceleration and are not an equilibrium or pure-jitter measurement.

## Cost and provenance

The first implementation's complete CPU steps, excluding offline audits and
export, cost 678/704 ms on average for original quiet/pull and 1,539/1,851 ms
for experimental quiet/pull. P95 is 717/745 ms versus 1,747/3,605 ms. These are
sequential single runs with two CPU workers, with noticeable timing variation;
they are not GPU frame times or a real-time claim. Retaining dormant EE pairs
increases incidence and AD work. A subsequent scalar support shortcut avoids
AD for identically zero contributions; it does not discard geometric pairs.

A subsequent 30-step pull/hold/release replay compares the archived and final
executables, excluding the first ten steps from timings:

| CPU implementation | Mean full step before | Mean full step after |
|---|---:|---:|
| Production-default model | 735.55 ms | 738.30 ms |
| Experimental model | 1,836.38 ms | 1,110.15 ms |

Both final states, including all positions, velocities and time, are
**byte-identical within each row**. The support shortcut saves 39.55% on this
short experimental replay. The experiment still costs about 50% more than
the default in that replay and is not promoted. Single-run CPU timings are
noisy and cannot predict hybrid GPU cost. The default comparison supports no
material regression in this test, not a universal performance guarantee.

Validation: 177 CPU tests, CPU/desktop all-target Clippy with warnings denied,
formatting and six template checks pass. Final targeted contact/VBD tests
were rerun after the evaluation shortcut, and the manifold-fixture/full-state
pull-release tests were rerun after strengthening the fixtures. The scalar
support shortcut matches full AD values, gradients and Hessians over 5,140
tested vertex blocks. No GPU validation of the new law is claimed.

Local evidence is under ignored `output/edge-transition-20261019`:
`capture-centers`, `synthetic.json`, four `vbd-*` runs, protocol/source/state
archives and `comparison.json`. Initial states share SHA-256
`18174fe9d1afdab39b95bd9dde0826b5e02874319f51776a1bbce179976f87df`.
The user's saved project hash is unchanged. No editor window, GPU workload,
commit, push or application replacement was part of this pass.

The useful outcome is a small reproducible failure, a falsifiable contact-law
experiment and release/collision gates. A global solution still needs coherent
transitions between all contact feature families without losing support near
boundaries or increasing the real-time cost beyond the budget.
