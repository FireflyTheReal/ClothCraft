# Coupled deforming contact and swept-path correction

The hard-contact prototype now handles reactions on **both sides of VF and
EE contacts** between two deformable cloth panels. The moving- and held-release
quality gates pass with accumulated path corrections and a larger nonlinear
iteration budget. The difficult case costs about **31 ms per step for only
eight vertices**. This is a small reference experiment, not a production
solver or a whole-dress jitter fix.

## Contact rows and continuous checks

`solver_lab/deforming_contact.rs` provides:

- Finite point/triangle and segment/segment distance Jacobians, including
  motion of every participating vertex. Closest-point weights distribute the
  reaction to both surfaces.
- A conservative certificate for straight vertex trajectories. A separating
  plane must separate every vertex of the two convex primitives at both
  endpoints of an interval. Their linear trajectories then remain separated
  throughout that interval.
- Adaptive subdivision when the separating plane rotates. Budget exhaustion
  returns `Unresolved`, never `Safe`. Sampling a few distances is not used as
  an acceptance proof.

The certificate has explicit numerical slack and a conservative floating-point
allowance. The experiment uses 1 mm clearance and 1e-10 m slack. It retains
the original OGC bounds in addition to these certificates. No candidate pair,
seam exclusion or collision bound is weakened.

At nonunique closest witnesses, the Jacobian is one selected limiting row;
smoothness is not guaranteed. Degenerate or ill-conditioned linearization
geometry is rejected. This remains an important limitation for general folds.

Four tests check full-coordinate finite differences at interior and boundary
features, rigid transformation invariance, internal force/torque balance,
common motion, sliding, VF/EE tunneling despite safe endpoints, and rotating
edges that require temporal subdivision.
The targeted CPU suite, including the earlier solver/contact regressions,
passes 41 tests.

## Eight-vertex experiment

`examples/deforming_contact_probe.rs` compiles two unpinned, unsewn panels:
a 60 mm square below a 30 mm square. Both deform using the existing membrane
and bending energies. The upper panel lands on the lower one, then one corner
is pulled sideways/upwards and released, either during motion or after a hold.
Each full protocol runs 180 steps at 1/60 s.

The initial query radius covers the entire finite geometry. All 49 eligible
cloth VF/EE identities are retained for the fixed topology, not used as a
near-contact temporal cache. All accepted paths must pass their certificates.
The planar floor uses explicit inequalities, and the original geometry
diagnostic runs at each completed step. Up to three EE reactions are active
in these runs, so this is not only a point/floor test.

The physical convergence check uses the current contact rows and the complete
force balance, complementarity and search displacement. Completing a time
step or reaching low kinetic energy is not sufficient.

## Failure and correction

The original working-set search passes held release but fails moving release:
80 of 180 steps exhaust the nonlinear budget. Collision clearance survives,
energy decreases, and velocity nearly vanishes, yet the force residual stays
large. This is artificial blocking, not successful settling.

The experimental correction evaluates violated swept constraints on a trial
path, linearizes them with respect to the complete coupled search displacement,
and solves another QP. These extra rows affect the **search path**, not the
physical force model used to declare convergence. The resulting proposal still
passes OGC bounds, full swept certificates and descent of the actual objective.
There is no projection applied after accepting a state.

Two correction policies were tested:

- Keep all previous path linearizations during the correction.
- Replace each pair's earlier row with its latest linearization.

Replacing old rows was a plausible way to avoid unnecessary restrictions, but
it performs worse here. Retain that negative result. Accumulating the rows
reduces failures to three steps at the original 96-iteration budget. Raising
the budget to 384 resolves the remaining steps; the largest observed count is
157, at a feature transition after moving release.

## Results

See [all measured gates](deforming-contact-validation.json). Failed controls
are deliberately retained, so that file's `all_quality_gates_passed` is false.
The two passing high-budget protocols are separately checked with
`--require-all` in the local `quality-gate.json`.

| Search / protocol | Iteration budget | Unconverged steps | Mean ms/step |
| --- | ---: | ---: | ---: |
| No path correction / moving | 96 | 80 | 99.70 |
| No path correction / held | 96 | 0 | 4.16 |
| Refreshed corrections / moving | 96 | 10 | 41.83 |
| Refreshed corrections / held | 96 | 0 | 1.45 |
| Accumulated corrections / moving | 96 | 3 | 25.54 |
| Accumulated corrections / moving | 384 | 0 | 31.11 |
| Accumulated corrections / held | 384 | 0 | 1.18 |

These are single CPU development-build samples, including diagnostics and
trace construction, excluding final serialization. Timing variability is
visible in the easy cases. Failed solves are not equivalent performance
baselines, and these numbers do not imply a production speedup.

For the two passing high-budget runs:

- Maximum current force residual: 3.99e-8 N.
- Maximum complementarity: 7.31e-19 J.
- Minimum cloth clearance: 1 mm within 4.6e-16 m.
- No measured post-release mechanical-energy increase.
- Original intersection/degeneracy checks and every accepted swept certificate
  pass.

Continued motion is allowed: no friction, sleeping, extra damping or velocity
filter is added. A moving release must not be forced to rest.

The expensive run performs 3,945 path-repair QP solves and 232,094
rank-deficient active-block fallbacks. Twenty-eight tentative repair QPs
exhaust their own budget; those failed proposals are not accepted as solved.
The final accepted physical states satisfy the full gates. This profile points
to rank-aware block solving and nonlinear convergence as the next targets,
rather than cheaper collision checks.

## Scope and next steps

This advances beyond the fixed-plane patch, but is still a tiny, unsewn
two-layer scene. Unequal seams were tested separately in
[the coupled-patch experiment](coupled-contact.md); the combined folded,
eased, self-contact problem has not passed. No new dress trajectory or GPU run
was performed, and the editor remains on its existing solver.

The [independent active-block follow-up](independent-contact-blocks.md) now
retains **all** inequalities and checks the same full residual. It reduces the
dual work and passes these release gates, but remains a dense small-scene
experiment. Next, compare against this dense reference
on folds, close layers, sewing stress and longer moving-release trajectories.
A sparse/matrix-free implementation and whole-dress quality gates remain
necessary before real-time claims.

## Reproduction

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test deforming_contact -- --test-threads=2
cargo build --locked --no-default-features -j 2 --example deforming_contact_probe
target/debug/examples/deforming_contact_probe.exe --swept-repair --accumulate-repair --iterations 384 --output output/deforming-contact-20261020/accumulated-repair-384.json
target/debug/examples/deforming_contact_probe.exe --swept-repair --accumulate-repair --iterations 384 --held --output output/deforming-contact-20261020/accumulated-repair-384-held.json
python scripts/summarize-deforming-contact.py output/deforming-contact-20261020/accumulated-repair-384.json output/deforming-contact-20261020/accumulated-repair-384-held.json --require-all --output output/deforming-contact-20261020/quality-gate.json
```
