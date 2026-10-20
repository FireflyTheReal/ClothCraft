# Long release tails and swept-path search

Research follow-up to [constrained contact search](constrained-contact-search.md).
Production VBD/OGC and the editor are unchanged.

## Longer runs falsify a broad stability claim

The four successful 180-step protocols establish only three simulated seconds.
Continue each exact frame 179 to a requested stop of 900 steps with the archived
constrained-search executable, unchanged physical and geometric thresholds.
The matching held case exhausts 384 nonlinear iterations at step 539; matching
moving fails at 369; the 10% eased held case fails at 780. These are convergence
failures, not accepted physical timesteps. The eased moving run fails at 489.

| Seam | Protocol | Failed step | Force residual (N) | Accepted tail steps |
| --- | --- | ---: | ---: | ---: |
| Matching | Held/released | 539 | 0.01066 | 359 |
| Matching | Moving/released | 369 | 0.00004953 | 189 |
| 10% eased | Held/released | 780 | 0.01044 | 600 |
| 10% eased | Moving/released | 489 | 0.009191 | 309 |

All four requested 15-second protocols therefore fail. Their accepted prefixes
show maximum energy rises between zero and 8.47e-15 J, which does not establish
full-run stability or convergence.

`scripts/summarize-contact-tail.py` separates mass-weighted rigid translation
and rotation of each materially connected component from its internal motion.
Triangles and sewing supports establish connectivity; transient contacts do
not merge components. Nonrigid speed and a velocity-difference highpass are
passive diagnostics, never sleep criteria or solver modifications. The failed
frame and every later frame are excluded from motion measurements. Incomplete
tails cannot pass the full physical, energy or quiet gates.

The matching accepted prefixes have no measured energy increase, but are not
quiet: final 60-step maximum nonrigid speed is 1.896 mm/s (held) and
0.889 mm/s (moving). This does not classify every remaining deformation as
spurious jitter. The same windows contain substantial legitimate rigid motion.
Energy comparisons include the checkpoint-to-first-tail boundary.

The tails are also edge-release tests, not purely stationary settling tests.
For example, matching/held frame 180 has upper-panel mean height 2 mm.
At frame 420 the upper panels already extend past the support's z footprint;
by accepted frame 538 their mean height is 1.6574 mm and their mean vertical
velocity is -2.65 mm/s. Falling and deformation as cloth leaves a support are
legitimate. The rising internal-motion curve alone is therefore not evidence
of artificial jitter; the diagnosed defect is failure to converge while
allowing that motion. The passive plot `original-tail-motion.png` separates
rigid motion and excludes unconverged frames.

## What blocks step 539

Passive `start_alpha`, `guard_vertex`, `backtracks` and accepted-iteration
line-search traces reproduce the archived failed positions and velocities
exactly. At the final iteration:

- OGC initially allows alpha 0.342825.
- Eleven further halvings leave alpha 0.000167395.
- Larger trials violate actual EE clearance or exhaust the swept certificate.
- The accepted trial decreases residual merit, but force residual remains
  0.01066 N versus the 1e-7 N acceptance threshold.

Thus removing the OGC prebound is not justified by this evidence. Endpoint
linearized feasibility alone fails to capture the curvature of the feasible
path. The swept checks correctly reject the larger motions.

## Opt-in experiment

`--newton-path-repair` adds swept-path linearizations to constrained residual
search, retaining the ordinary candidate as a comparison. At a rejected path
sample time `t`, the distance Jacobian gives a search row
`J dx >= (clearance - distance(sample))/t + J dx_trial`.
At most eight rounds accumulate such rows. These constraints acquire no
physical reaction variables and do not alter the force/complementarity model.
They are only search proposals: original OGC clipping, all retained VF/EE
swept certificates, floor checks and actual nonlinear merit remain mandatory.

All four formerly failed isolated steps now converge using that same policy:

| Seam / protocol | Step | Iterations | Force residual (N) | Mechanical energy change (J) |
| --- | ---: | ---: | ---: | ---: |
| Matching / held | 539 | 59 | 4.01e-9 | -1.900e-7 |
| Matching / moving | 369 | 54 | 4.02e-9 | -1.061e-7 |
| Eased / held | 780 | 103 | 4.41e-11 | -2.976e-8 |
| Eased / moving | 489 | 51 | 1.90e-10 | -4.279e-8 |

These replay exact earlier accepted checkpoints, not the whole preceding path
with the new policy. They retain the unchanged swept/floor/force/complementarity
gates. Costs are 36–54 seconds per isolated difficult step in these development
runs. This is a convergence improvement, not a performance claim.

All four fresh 180-step protocols also pass the unchanged geometry, force,
complementarity and post-release energy gates with the path policy. Matching
held/moving positions and velocities are exactly equal to the earlier policy
throughout those short runs. Eased trajectories differ; all four retain
7.89–8.43 mm displacement RMS in the first ten release steps. This excludes a
freeze-based success but does not establish perceptual fluidity.

The probe remains a dense finite-difference CPU oracle. The longer eased moving
tail with the new policy also fails: step 479 exhausts 384 iterations with
force residual 8.29067e-5 N and complementarity 8.46104e-8 J. Steps 180–478
are accepted; the failed diagnostic frame is excluded from motion analysis.
The accepted prefix's maximum energy rise is 6.73e-15 J, but full-run
convergence, energy and quiet gates are false. This is a new trajectory, so
rescuing the old isolated step 489 did not establish longer-run robustness.
No realtime or dress fix is claimed. The path option conflicts with bounded fallback, whose lazy candidate
semantics are not implemented for repaired proposals.

### Exact residual-sample reuse

`--reuse-newton-samples` is an opt-in CPU cost experiment. It memoizes the
residual evaluations used by finite-difference and unit-reaction columns
within one method at one nonlinear iterate. Positions and all reaction
multipliers are keyed by their exact f64 bits, including signed zero. The
cache is discarded before a new method/iterate; the material, inertial target
and mouse control are fixed during its lifetime. Failed evaluations are not
cached. Beyond 1024 retained samples, computations proceed without storing
new entries. This limit never removes a contact or suppresses an evaluation.

The line search, path certificates, QP/least-squares arithmetic and nonlinear
acceptance tests are unchanged. Per-iteration diagnostics count requests and
actual residual evaluations. Exact trajectory comparisons and timing are
required before interpreting this as a useful optimization.

All four cached isolated repairs preserve **every pre-existing frame,
iteration and line-search field exactly**, not just final positions. Residual
evaluations fall from 43,034–107,576 requests to 6,146–12,137 evaluations
(85.7–88.7% avoided). Three cache unit tests cover exact f64 keys/values,
errors, disabled/new-iterate behavior and the storage limit.

A sequential off/on/on/off comparison uses the same profiled executable,
one Rayon thread and the same eased-moving step 489 checkpoint:

| Run | Whole-process CPU time (s) | Probe elapsed time (s) | Residual evaluation time (s) |
| --- | ---: | ---: | ---: |
| Off A | 46.188 | 50.515 | 9.575 |
| On A | 31.484 | 32.022 | 0.986 |
| On B | 29.078 | 29.912 | 0.835 |
| Off B | 33.625 | 34.320 | 6.087 |

That is about 14–32% lower CPU cost in the two adjacent comparisons, with
substantial drift between the uncached samples. Background workstation load
and the older long-tail process are uncontrolled. This is not a general
speedup guarantee or an interactive FPS measurement. With the cache, path
search still takes 20–21 seconds and dominates cost; swept repair geometry
takes 0.19–0.21 seconds in these samples. The next performance target is the
constrained linear search, not removal of collision certificates.

Evidence: `output/contact-tail-20261020`, including the unchanged archived-binary
tails, exact-step diagnostic replays and passive analyses. The previous
three-second results remain valid and separate.

[Machine-readable validation](contact-long-tail-validation.json) records the
completed gates and hashes. The repaired tail has finished, with the failure
above. Its output is `output/contact-tail-20261020/path-tail-ease0.1-moving.json`,
resumed from the new policy's frame 179, not from an old-policy tail. Its
original executable was relocated intact to `uncached-running-probe.exe`
without stopping or restarting the process.
The current profiled/cache build is archived separately as `path-probe.exe`,
with 215 source files and SHA-256 records in `build.json`. Both reproduction
scripts parse successfully. Keep these archives unchanged.

The cached complete 180-step eased-moving run also preserves all pre-existing
frame, iteration and line-search fields exactly and passes the same quality
gate. This cache avoids repeated computation; it does not repair convergence.

The final isolated follow-up uses [moving separating planes](moving-contact-separator.md).
It certifies some rotating paths that exhausted the fixed-plane budget, with
the same clearance and rejection of unresolved intervals. Replaying accepted
frame 478 now converges at step 479 in eight iterations. This is one isolated
rescue, not a completed long trajectory. Further experiments are paused at
the owner's request.
