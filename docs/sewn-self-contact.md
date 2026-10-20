# Sewn, folded cloth: convergence and contact work

Follow-up: [an invariant discrete contact row](discrete-contact-work.md) passes
offline work and sliding checks on captured states. It is not yet integrated,
so the full moving-release failures below remain unresolved.

The combined fixture exposes two different problems. A dual active-set update
can stall a feasible contact QP; the new boundary-pivot method fixes the
captured example and completes the previously stalled time step. Separately,
some fully converged moving-release trajectories gain mechanical energy
through discrete contact work. The complete quality gate therefore still
fails. Production VBD/OGC and the editor are unchanged.

See [measured gates and energy balances](sewn-self-contact-validation.json).
This follows the [independent-block experiment](independent-contact-blocks.md).

## Fixture and replay

`deforming_contact_probe --sewn-ease 0.1` creates two initially folded panels
with a 10% seam-length mismatch above a third, deformable panel. There are
16 vertices, one seam and 266 eligible VF/EE pairs. `--sewn-ease 0` is the
matching-length control. The fold is 20 degrees on each side by default.

The fixture uses existing material/sewing energies, unchanged OGC movement
bounds, conservative swept certificates and the existing local seam
exclusions. A test verifies initial geometric feasibility and identical
material coordinates/masses under different placement angles. No pinned
vertices or garment-specific rules are used.

Both moving release (step 100) and held release (step 130) run to step 180.
`--progress`, `--audit-iterations`, `--stop-on-unconverged` and failed-QP
capture make long failures inspectable. `--resume <trace> --resume-after <step>`
requires a converged frame and matching fixture/protocol metadata. It restores
positions/velocities only; the original fixture still defines material rest,
contact topology and reference geometry. A partial replay is not a complete
protocol pass.

## A feasible QP that failed to converge

The first full eased moving-release run was deliberately stopped after about
four minutes because it had no intermediate output. It is recorded as
interrupted, not passed or failed physics. The instrumented run then completed
166 converged steps and exhausted 384 nonlinear iterations at step 166.

At that step, the physical QPs converge in a few sweeps. Most path-repair QPs
do not, forcing tiny accepted steps. The final force residual is about
1.06e-3 N despite preserved clearance.

One failed repair QP is now the public regression fixture
`tests/fixtures/sewn-contact-qp.json`: 48 coordinates and 26 inequalities,
generated solely from the synthetic scene. Its zero displacement already
satisfies feasibility to the solver's 1e-11 m tolerance. A separate SciPy
HiGHS feasibility check also succeeds. Increasing the previous coordinate,
active-block or independent-block methods to 32,768 sweeps still does not
meet all KKT tolerances.

`IndependentPivotBlock` finishes conditional active-block solves after a
multiplier reaches zero, before coordinate updates can reactivate it. The
blocking multiplier is assigned its exact boundary value zero, avoiding
cancellation that can leave a tiny positive value. Constraints remain in the
problem and can become active again in later sweeps. There is no small-force
cutoff, added compliance, changed contact tolerance or position projection.

On the captured QP it converges in two outer sweeps and 11 boundary pivots,
with maximum feasibility error 2.05e-18 m and complementarity 1.36e-20 J.
The old methods fail their 8,192-sweep budget. The isolated nonlinear step
then converges in 27 iterations with force residual 7.39e-9 N, although five
other tentative repair QPs still exhaust their budgets. This does not solve
every contact QP or make the dense method scalable.

## Full protocols

| Seam / release | Completed, converged steps | Geometry gate | Energy-decay gate | Mean ms/step |
| --- | ---: | --- | --- | ---: |
| 10% ease / moving | 180 | Pass | **Fail** | 279.58 |
| 10% ease / held | 180 | Pass | Pass | 14.41 |
| Matching / moving | 180 | Pass | **Fail** | 68.65 |
| Matching / held | 180 | Pass | Pass | 5.61 |

These are single CPU development-build measurements including diagnostics.
There are only 16 vertices; these costs are unsuitable for garment realtime.
All accepted paths retain collision checks. Completing the protocols and
meeting force balance do not erase the failed energy gates.

The earlier eight-vertex, unsewn moving/held protocols still pass. A scalar
energy-only path avoids constructing unused derivatives during line searches.
It uses `Dual<0>` to retain the assembly's floating-point arithmetic, including
reciprocal multiplication for division. A raw-f64 trial had ulp-level energy
differences and was replaced. The final scalar/full-assembly frame traces
match exactly on both old protocols.

## Energy accounting: what adds energy?

`--audit-energy` is observational. On each converged step it records the
following discrete identity, using the actual end-of-step forces:

```
Delta mechanical energy =
    material remainder
  + contact work
  + grab work
  + force-residual work
  - backward-Euler kinetic dissipation
```

With displacement `dx`, material energy `U`, end material gradient `grad U`,
contact reaction `J^T lambda`, mass `M` and velocity change `dv`:

- Material remainder: `U_new - U_old - grad U_new . dx`.
- Contact work: `(J_new^T lambda) . dx`.
- Kinetic dissipation: `0.5 dv^T M dv`.
- Force-residual work: the actual stationarity residual dotted with `dx`.
- Grab work: the applied grab force dotted with `dx`; zero after release.

Gravity is included in mechanical energy and cancels exactly as a linear
potential in this identity. The measured closure error is at most 1.09e-19 J.
The diagnostic now includes the **first** released step; the earlier 18
independent-block traces still pass that stricter accounting window.

At the largest positive change in the matching moving-release run (step 176):

| Contribution | Joules |
| --- | ---: |
| Contact work | +1.62948e-6 |
| Material remainder | -2.57893e-7 |
| Implicit dissipation | -2.86233e-7 |
| Force-residual work | +9.16e-14 |
| Mechanical-energy increase | **+1.08536e-6** |

The eased moving-release run has the same issue at smaller magnitude:
peak increase 1.42535e-8 J at step 163. Its contact work is +5.79017e-8 J,
larger than the negative material remainder and implicit dissipation combined.

Thus, in these research trajectories, positive discrete contact work explains
the energy increase even with a small force residual. Stored seam energy is
not arbitrarily removed. This is not proof that every production jitter event
has the same cause: the research hard-contact model differs from the installed
OGC barrier potential, whose activation jumps are documented separately.

## Validation and next work

The targeted suite passes 46 CPU tests, including the captured QP, seeded
comparisons against an independent small QP oracle, coupled material
derivatives, contact gradients, swept tunneling and initial fixture geometry.
CPU all-target Clippy and formatting pass. No GPU was used.

The next numerical target is a contact time discretization with controlled
discrete work, while retaining momentum consistency, collision safety and
free sliding/release. Measure it against this energy decomposition, not only
quiet-frame motion. Remaining expensive repair QPs also need independent
captures. Do not promote this prototype or hide its energy rise with sleep,
extra damping, velocity filtering or a post-step energy clamp.

## Reproduction

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo build --locked --no-default-features -j 2 --example deforming_contact_probe --example dense_qp_replay
target/debug/examples/deforming_contact_probe.exe --independent-block --pivot-block --swept-repair --accumulate-repair --iterations 384 --sewn-ease 0.1 --progress --audit-energy --stop-on-unconverged --output output/sewn-self-contact-20261020/replay.json
```

Add `--held` for held release; use `--sewn-ease 0` for matching lengths.
Omit `--pivot-block` to exercise the previous method. The complete quality
summary deliberately fails the moving sewn protocols:

```powershell
python scripts/summarize-deforming-contact.py output/sewn-self-contact-20261020/replay.json --require-all --output output/sewn-self-contact-20261020/replay-summary.json
```

`dense_qp_replay --input <trace> --output <result> --sweeps 8192,32768`
compares all four dual methods on the captured `failed_repair_qp`; it never
advances a simulation or declares the returned displacement collision-safe.
