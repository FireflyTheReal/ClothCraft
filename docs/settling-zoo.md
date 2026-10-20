# Settling zoo: motion, force balance and release

CPU research, 19 October 2026. `examples/settling_zoo.rs` runs the actual f64
VBD/OGC solver without a window. It does not freeze vertices, change material
rest shapes, zero velocities, add damping or bypass collision detection.
Production defaults and the desktop executable are unchanged.

## Reproduce

```powershell
.\scripts\settling-zoo.ps1 -Output output/zoo-baseline
.\scripts\settling-zoo.ps1 -Output output/zoo-search -Backtracks 32 `
    -Cases seam-eased,floor-face,floor-diagonal,slope -ControlsFrom output/zoo-baseline
.\scripts\settling-zoo.ps1 -Output output/zoo-combined -Backtracks 32 -UnshiftedSpd `
    -Cases seam-eased,floor-face,floor-diagonal,slope -ControlsFrom output/zoo-baseline
```

The wrapper builds with `--no-default-features -j 2`, archives the executable,
hashes and arguments, and refuses to overwrite evidence. The runner uses two
Rayon threads. There is no GPU path in this example.

Portable equivalent:

```sh
cargo run --release --locked --no-default-features -j 2 --example settling_zoo -- --output output/zoo-baseline
```

`--cases`, `--seed`, `--resolution`, `--iterations`, `--settle-seconds` and
`--continuation-seconds` select the experiment. Seed zero is canonical; other
seeds vary panel width, height and easing, deterministically. This is a small
family of physical fixtures, not exhaustive fuzzing of arbitrary meshes. The
separate [directional geometry zoo](directional-contact.md#headless-laboratory)
covers randomized geometric safety configurations.

## Fixtures and controls

| Fixture | Purpose |
|---|---|
| `zero-gravity` | Unstrained free cloth should retain rest; release can move its center of mass |
| `free-fall` | Negative control: low oscillation must not be mistaken for rest |
| `hanging` | Two pinned top corners, gravity, no avatar |
| `seam-equal` | Two panels, equal-length sewing and two outer pins |
| `seam-eased` | 15% unequal length and nonmatching edge discretization at seed zero |
| `floor-face` | Horizontal cloth over the interior of one static triangle |
| `floor-diagonal` | Same cloth over two coplanar triangles with an internal diagonal |
| `slope` | Five-degree support without friction: sliding is expected, not quietness |

All cases retain self-contact. The floor is explicit collision geometry, not
the editor's visual floor. The slope extends to +/-200 m to avoid confusing
falling off a short test support with tangential contact behavior.

Defaults: 20 s draping, then four separate 10 s continuations of exactly the
same state. `continuous` clones solver history; `cold-restart` reconstructs it;
`fresh-contacts` reconstructs only the contact world while retaining prediction;
`pull-release` clones history, moves a barycentric attachment over 1 s, holds
0.5 s, then releases for 8.5 s. The target offset is (20, 25, 40) mm with a smooth
cubic ramp and a 100 N/m attachment. No control is active in the final window.

By default, each run grabs its own draped position. Use `--controls-from` for
the same absolute world-space target path across variants; the source template
and support must match. Different solvers can arrive at different pre-grab
states even with matched controls. This does not imply identical animation.

## Measurement contract

Every accepted step records full solver cost, mass-weighted RMS speed, maximum
speed, high-pass displacement, fresh physical forces on unpinned vertices,
fixed-contact energy accounting and contact-refresh energy/force differences.
Audit work and file I/O are outside `step_ms`; total laboratory runtime is not
an interactive performance claim. The audit detector is separate from the
solver, so it does not perturb live contact caches.

The final two seconds determine the label:

- `quiet-observed`: every vertex's trajectory fits within a 10 micrometre AABB
  diagonal, maximum speed stays below 0.1 mm/s, and RMS physical force stays
  below max(1 nN, 1% of RMS vertex weight).
- `quiet-with-unbalanced-forces`: motion passes, force balance does not. Inspect
  truncation counts and `final-vertices.json`; this may be blocked cloth.
- `still-moving`: at least one motion threshold fails. This includes legitimate
  drift and decaying oscillation; it is **not** automatically numerical jitter.
- `insufficient-observation`: less than the required interval was observed.

These are explicit diagnostic thresholds, not sleep thresholds or a proof of
equilibrium. Small force error at one configuration does not certify stability
under future interactions. Truncation counts include rejected search trials;
they are not unique vertices or certified constraint reactions. Contact-force
jumps compare old/new features at the same final position and can include
changes in their representation. The energy identity's closure tests accounting,
not physical correctness; do not sum grab phases without control work.

Each continuation saves its initial/final state and validates the final surface
with the independent intersection diagnostic. Existing OGC feasible regions,
seam exclusions and floating-point limitations still apply. A final check is
not continuous collision certification of the entire trace.

State JSON is a laboratory snapshot paired with its template, support and
settings; it is not an editable `FittingProject`. The original Garment is never
opened or modified by this zoo.

## Experimental local solves

Both probes are opt-in through `VbdSolver::set_local_solve_probe`, with no
document field or UI toggle. Default remains ten search trials and the existing
Gershgorin shift. The WGSL solver is unchanged; these are CPU comparisons.

1. **Longer backtracking:** allow up to 32 trials, stopping at the first accepted
   candidate, with the same objective, tolerance and collision bounds. A contact
   onset unit test reproduces ten rejected trials followed by a safe smaller
   energy-decreasing step. This addresses search exhaustion, not convergence
   of an entire material patch.
2. **Unshifted SPD block:** attempt a 3x3 Cholesky solve on the actual local
   Hessian plus inertia. If it is not sufficiently positive/finite, use the old
   shifted fallback. A rotated positive-definite contact block can have a
   negative Gershgorin bound; shifting it unnecessarily also stiffens tangent
   directions. The probe changes the search direction, not the physical force.
   It still uses the same line search and feasible region.

The algebraic test checks the rotated block's residual and tangent response,
including indefinite/ill-conditioned rejection. These probes must not be enabled
in production based on lower jitter alone. Required follow-up includes the
private dress checkpoint, matched pull/release, contact-rich folds, mesh/scale
variation, GPU parity, and end-to-end cost.

## Research context

[Chen's VBD damping note](https://ankachan.github.io/posts/2026/03/implementing-vbd-properly/)
explains why diagonal stiffness applied to absolute velocity can suppress rigid
motion. We have not added that damping. The present probes address local
minimization and initialization instead.

[Divide and Truncate](https://arxiv.org/html/2604.15513v1) separates safe motion
regions from the quality of the finite-iteration solve; its limitations also
discuss energy behavior. That distinction motivates measuring force balance and
release, rather than treating collision safety or low motion as sufficient.

See [the dress checkpoint investigation](dress-checkpoint-and-jitter.md) for the
real garment's contact-refresh and residual-work evidence. Results from these
small CPU fixtures do not establish that its hybrid GPU jitter is fixed.

## Findings, 19 October

Canonical runs are archived under `output/zoo-final-{baseline,search,combined}`.
The search and combined variants replay the baseline's absolute grab targets.
The tables use the final two seconds of each continuation, at 60 Hz and 16
iterations. Values are single-run CPU measurements, not GPU predictions or
confidence intervals.

### Unequal sewing can reach rest

The unmodified solver reaches near-machine-precision force balance for the
suspended equal and unequal seams. The eased pair's continuous continuation
has a maximum two-second vertex span below 1e-12 m and mean physical force RMS
8.92e-15 N. Stored sewing energy is allowed to remain. Unequal sewn lengths alone
therefore do not imply sustained jitter in this fixture.

Cold reconstruction of the same eased state creates a 0.1304 m/s peak
mass-weighted RMS speed in the first 20 steps. On the first step alone,
fixed-feature mechanical energy rises by 0.2295 mJ; contact-refresh energy is
zero. Rebuilding only the contact world yields an identical final state to
continuous execution. Together with the initialization code, this isolates the
lost adaptive predictor history as a source of the restart disturbance in this
case. It does not establish the origin of every ongoing dress oscillation.

Extending the unmodified eased-seam release to 28.5 s after detachment reaches
a final two-second span of 7.66e-12 m and force RMS 2.37e-14 N. Its short-run
motion was a decaying transient, not a sustained limit cycle. The cold restart
also eventually settles. A persistent real-dress jitter therefore needs a more
demanding fixture or the dress itself; this simple tension case does not
reproduce it indefinitely.

### Quiet contact can hide failed search

After the same pull/release:

| Fixture | Default physical force RMS | 32-trial physical force RMS | Default / 32-trial mean step |
|---|---:|---:|---:|
| One support face | 2.992e-3 N | 1.217e-14 N | 2.38 / 1.62 ms |
| Coplanar support diagonal | 2.045e-4 N | 2.555e-10 N | 2.15 / 1.73 ms |

Both default runs pass the motion threshold but fail the force gate. The longer
search removes the force imbalance without enlarging collision regions. It can
also save work overall by avoiding repeated unsuccessful searches; a larger
maximum trial count does not mean that every vertex executes all trials.
The search-only eased-seam trajectory is unchanged: this is not a blanket jitter
remedy. All these final intersection diagnostics pass.

Seed 7 changes the sizes and easing: the two support release force residuals
again drop from approximately 2.95e-3 / 8.13e-4 N to 1.34e-14 / 1.40e-10 N.
This is evidence beyond one hardcoded layout, not a broad randomized guarantee.

### Better local directions are not a complete contact solution

On the large five-degree support, baseline sliding averages 0.93 mm/s, whereas
the combined unshifted-SPD/long-search probe averages 83.23 mm/s. Neither matches
the frictionless mass-drag terminal speed `g*sin(5 degrees)/1.5`, approximately
570 mm/s. This exposes substantial remaining finite-iteration resistance to
sliding. Do not mistake increased motion for a validated physical solution.

The net physical downhill force divided by mass at the final state is about
0.854998 m/s² in all three variants, matching `g*sin(5 degrees)`; it is the
finite solve that does not realize the expected motion in this fixture. The
free-fall negative control likewise moves too slowly on the long baseline
(about 0.353 m/s mass RMS versus the 6.54 m/s mass-drag terminal speed). The zoo
flags motion but does not call that physical-accuracy success. These are CPU
findings; do not extrapolate their magnitudes to the hybrid backend.

The unshifted-SPD-only preliminary run worsened force-imbalanced rest on the
support diagonal (about 0.0293 N RMS). The combined probe needs the longer
search to avoid that failure. On the eased seam's release tail, the combined
probe reduces vertex span from 156 to 95 micrometres, but still has positive
fixed-feature energy steps. It is not a demonstrated energy-monotone solver.

Preliminary `output/settling-zoo-20261019-*` runs used each variant's own grab
anchor. Their slope had a shorter support and some late trajectories left it;
do not use those late slope values as steady sliding evidence. The final runs
enlarge the support and match controls explicitly.

### Next gates

1. Test a force-informed cold predictor or a validated history checkpoint, while
   retaining the full gravity/inertia objective. Preserve exact free fall and
   promptly respond to edits and grabs. Do not resurrect the rejected objective
   guard that made cloth sticky.
2. Replay the longer-search candidate on the dress from its draped checkpoint,
   with matched rest and pull/release. Attribute exhausted searches and final
   force residuals; do not enable it because a motion statistic is lower.
3. Resolve the sliding/convergence deficit before promoting unshifted blocks.
   Add mesh/scale variation and compare full coupled material modes where the
   residual requires them.
4. Only then port a demonstrated change to WGSL and measure full GPU step costs,
   conservative contact parity and interaction behavior. No GPU was used here.

## Validation record

[Compact machine-readable evidence](settling-zoo-validation.json) records the
six final batches: 87,600 accepted CPU steps and 115 passing final intersection
checks. Baseline/search/combined use matched world-space grab anchors; seed 7
has its own matched pair. Twenty-eight final default states are byte-identical
to the executable built before adding the opt-in probes. This checks default
behavior, not timing equivalence.

`scripts/check-cpu.ps1` passes: formatting, 167 CPU tests, CPU all-target Clippy
with warnings denied, and all six template validations. Two additional example
tests pass for the classifier and 24 seeded fixture initializations. The new
core tests cover rotated SPD blocks and exhausted contact-onset search while
preserving energy descent and the original feasible region.

`scripts/summarize-settling-zoo.py` aggregates saved runs without altering them.
`peak_grab_displacement_m` measures distance from the chosen control anchor;
with matched controls this is not necessarily displacement from that variant's
own initial attachment, and it is not grab tracking error.

No editor launch, GPU workload, production activation, commit or push was part
of this pass. The original garment and its saved state were not modified.
