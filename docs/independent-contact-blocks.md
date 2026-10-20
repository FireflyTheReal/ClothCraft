# Redundant contact constraints: independent dual blocks

Follow-up: [combined sewn self-contact](sewn-self-contact.md) resolves one
active-set stall with boundary pivots, but exposes an energy-decay failure in
fully converged moving-release trajectories. The smaller passing cases below
do not override that stronger negative result.

This is a CPU research optimization of the [coupled deforming-contact
experiment](deforming-contact.md). It is not enabled in the editor or the
production VBD/OGC backend. It does not establish that the saved dress's jitter
is fixed.

## What changes

The previous active-block method tries to solve all positive contact
multipliers together. Dependent distance Jacobians make that block singular;
the solver then relies on slower coordinate updates. The moving-release
fixture generated 232,094 such fallbacks.

`DualMethod::IndependentBlock` selects independent rows with incremental
Cholesky in the `H^-1` metric. It solves their conditional dual problem while
holding the remaining multipliers fixed. Every constraint still participates
in coordinate sweeps and the complete feasibility, stationarity and
complementarity checks. A stronger parallel inequality cannot disappear
because another row was selected first.

The pivot threshold, `1e-12` relative to the row diagonal, determines block
membership only. It neither softens the contact nor changes the residual
tolerances. Numerical failure returns to coordinate updates. Exhausting the
budget remains an explicit nonconvergence.

Within one immutable QP, a lazy symmetric cache reuses entries of
`J H^-1 J^T`. It retains the uncached arithmetic expression and is discarded
between QPs. Allocation is capped at 1 MiB; larger systems use uncached
arithmetic without dropping rows. `IndependentBlockUncached` provides an
explicit comparison path.

## Quality checks

Evidence is recorded in [the validation summary](independent-contact-validation.json);
private run artifacts are under `output/independent-block-20261020/`.

Both 180-step protocols pass: releasing while the attachment target is moving,
and releasing after a hold. The scene contains eight deformable vertices,
49 eligible cloth VF/EE pairs and a floor. It uses the existing material
energies, OGC bounds, complete swept-path certificates and accumulated path
correction at the same 384-iteration budget.

- Cached and uncached frames are exactly identical, excluding cache counters.
- Compared with the previous active-block method, maximum position difference
  is 2.06e-9 m and maximum velocity difference is 3.80e-9 m/s.
- Maximum physical force residual is 3.99e-8 N; maximum complementarity is
  7.19e-19 J.
- Minimum separation remains 1 mm within floating-point tolerance.
- No post-release mechanical-energy increase is measured.
- No sleeping, extra damping, velocity filtering or material-rest change is
  used. Continued frictionless motion is allowed.

The difficult protocol reduces total dual sweeps from 396,172 to 214,824 and
eliminates singular-block fallbacks. It still needs 1,180 outer iterations
across 180 steps. Thirteen tentative path-repair QPs exhaust their budget,
versus 28 for the control; these are not accepted as converged physical states.
All final accepted states pass the complete gates.

The cache computes 337,961 Schur entries instead of 11,226,822, with
10,888,861 hits and at most 3,720 bytes of cache storage in the moving-release
fixture. This operation-count reduction is not a corresponding speedup of
the complete simulation.

Three sequential repetitions per method, with rotated method order and no
simultaneous build/test job, give these median complete-probe costs:

| Protocol | Previous active block | Independent, uncached | Independent, cached |
| --- | ---: | ---: | ---: |
| Moving release | 24.23 ms | 21.32 ms | 17.88 ms |
| Held release | 1.29 ms | 1.26 ms | 1.36 ms |

These are development-build CPU measurements, including diagnostics and trace
construction. The difficult case improves by about 26%; the easy case does
not benefit from caching. The difficult case still exceeds 16.67 ms despite
its tiny size. Do not infer garment throughput or a universal speedup.
All 18 repeated trajectories pass the quality gates; each method reproduces
its own frame trace exactly across repetitions.

Eight dense-contact tests cover redundant/scaled rows, dependent and nearly
dependent inequalities, infeasibility, memory-budget fallback, and exact
cached/uncached agreement against an independent three-coordinate oracle.
The targeted contact/energy regression suite passes 44 tests.

## Limits

This remains a dense reference solver limited to 192 degrees of freedom.
Independent block selection does not solve every ill-conditioned contact
problem or guarantee convergence within a fixed budget. The repair loop and
nonlinear convergence remain expensive. Folds with unequal seams, large
contact sets and the complete saved dress still need matched quality gates
before a scalable implementation or production change.

## Reproduction

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test dense_contact -- --test-threads=2
cargo build --locked --no-default-features -j 2 --example deforming_contact_probe
target/debug/examples/deforming_contact_probe.exe --independent-block --swept-repair --accumulate-repair --iterations 384 --output output/independent-block-20261020/cached-moving.json
target/debug/examples/deforming_contact_probe.exe --independent-block --no-schur-cache --swept-repair --accumulate-repair --iterations 384 --output output/independent-block-20261020/uncached-moving.json
```

Add `--held` for the held-release protocol. Omit both independent-block flags
for the previous active-block control. Run `scripts/summarize-deforming-contact.py`
on the resulting files with `--require-all --output <summary.json>`; process
completion alone is not a quality pass.
