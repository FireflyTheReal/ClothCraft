# Directional contact bounds and the headless physics lab

The 19 October change releases the near-collapsed faces diagnosed in
[the ruffled dress investigation](collapsed-triangle-stalls.md). It changes
contact feasibility regions, not material rest geometry, seam energies, mass,
damping or the time step. It is not a general jitter cure.

## Why a safe direction was blocked

The original OGC ball uses the closest included primitive distance in every
direction. An almost flat triangle can put a vertex micrometres from an adjacent
face, even when their contact force is rejected by the feasible-region test.
The ball then prevents tangential motion as well as penetration. Removing the
pair, flooring its distance or excluding the whole panel would remove collision
coverage. None of those changes is used here.

For pairs closer than `query_radius / 64`, `ogc/directional.rs` attempts a
separating plane. The normal comes from the closest points, but separation is
verified against **all vertices of both primitives**. The plane lies midway
between their support intervals. Each moving vertex gets a half-space keeping
it on its original side, with a remaining margin. Other pairs retain their
original scalar bounds. A vertex may have several half-spaces and a residual
ball; all must hold simultaneously. A force rejection never removes a bound.

This is an independent sparse, fixed-plane implementation informed by the
supporting-plane construction and query-envelope argument in
[Divide and Truncate, sections 3.3–4](https://arxiv.org/html/2604.15513v1).
It does not implement their adaptive plane placement or DAP projection, and it
does not add an AVBD, IPC or other solver backend.

## Safety argument and floating-point scope

Let the verified support gap be `g > 0`, and `gamma = 2 * relaxation < 1`.
For a vertex at signed distance `s` from the midpoint plane, its permitted
normal displacement is `gamma * s - error`. Thus its remaining signed distance
is at least `(1 - gamma) * s + error`. Convex combinations preserve the same
side of the plane, so the entire vertex/face or edge/edge pair stays separated.
The regions are convex and fixed until detection refreshes them; line segments
between feasible iterates remain inside them.

Every vertex also retains a radius at most `relaxation * query_radius`.
Consequently two primitives initially farther apart than the query radius
cannot close that separation before detection refreshes. Nonconverted pairs
keep the original smaller distance-based radii. Reciprocal VF, EE, candidate
cache coverage and local seam exclusions are unchanged.

The separator uses f64. Its numerical reserve scales with world coordinates;
uncertain or nonpositive support separation falls back to the original ball.
This is a floating-point conservative construction with regression evidence,
not exact geometric predicates. Existing initial-intersection tolerances and
seam exclusions still apply. Already intersecting states are not repaired.

## Solver integration

`VbdSolver::new` enables the directional path. Raw `OgcWorld` construction stays
isotropic so geometric diagnostics can reproduce the baseline. The replay
example explicitly selects either mode, independent of the production default.

The CPU uses the same local implicit objective and energy-decreasing line
search. For a directional vertex, clipping follows the proposed Newton segment
from the **current iterate**, within the region centered at the **original
detection**. Clipping the whole displacement from the detection center would
undo accepted tangential motion and introduce artificial resistance.

The hybrid GPU path solves only the sparse directional vertices on CPU in f64,
alongside its existing precision fallback. They receive the same adaptive
initial guess as GPU vertices, then two dependency-ordered local sweeps. They
are held fixed during GPU work; all contacts and their incidence remain present.
Ordinary vertices use the existing GPU kernels. CPU-oracle validation compares
the complete half-space arrays as well as contact terms and radii.

`directional_vertices`, `precision_vertices` and `precision_ms` expose the
cost. There is no silent cap on the number of constrained vertices. Dense
micrometre contacts may make this CPU fallback expensive; that remains a case
to measure before considering a GPU plane solver.

## Headless laboratory

No desktop window is needed. The public randomized geometry zoo is CPU-only:

```powershell
.\scripts\physics-lab.ps1 -Cases 512
```

It records its seed, generates VF/EE configurations with varying gaps, rotations
and translations, compares scalar/SIMD detection, and checks separation along
sampled accepted trajectories. Additional fixtures cover tangential motion,
uncertain separators and the unseen-pair query envelope. Sampling supplements
the convexity argument; it is not continuous collision certification.

With explicit GPU authorization, add `-Gpu` to replay a public unequal-length
seam fixture. A private garment can be replayed without modifying its save:

```powershell
.\scripts\physics-lab.ps1 -Gpu -Project path/to/garment.clothcraft.json `
    -Panel panel_id -Point @(0.1, 0.2) -Steps 360
```

`Point` is in the selected panel's material coordinates, in metres. It selects
the attachment triangle. All physical settings and the saved mesh are kept.
The standard sequence is 120 settle steps, 60 pull steps (40 mm), 30 hold steps
and 150 release steps. The baseline generates mouse targets; the candidate and
its separate CPU-oracle run replay the exact same targets. This comparison can
apply different relative loads once trajectories diverge. A second candidate
run without `--controls-from` tests a natural pull from its own settled state.

Outputs include full trajectories, controls, energy audits, area ratios,
contact/step metrics, source hash and a comparison JSON. Output directories
must be new. User geometry and traces stay under ignored `output/`; public
fixtures contain no private garment data. The complete solver-step timer
excludes offline audit/export work. Oracle timings are validation only.

## Motion gates and remaining limits

### Ruffled dress replay, 19 October

The private capture contains 5,126 cloth vertices and 9,546 faces. Saved
materials, 16 iterations and the 1/60 s step are unchanged. Runs were sequential
on an RTX 3090 (Vulkan, NVIDIA 616.56), with two CPU workers and no window.
[Machine-readable measurements](directional-contact-validation.json).

| 360 steps / 6 simulated seconds | Isotropic baseline | Directional |
|---|---:|---:|
| Full step, mean | 20.13 ms | 19.66 ms |
| Full step, P95 | 23.20 ms | 24.30 ms |
| Full step, maximum | 34.25 ms | 40.52 ms |
| Release phase, mean step | 19.82 ms | 17.82 ms |
| Mean displacement of nine restricted vertices after 120 steps | 0.403 mm | 105.43 mm |
| Faces below 1% of rest area at the end | 2 | 0 |
| Minimum final area / rest area | 0.000220 | 0.729 |
| Maximum final sewing gap | 85.12 mm | 3.38 mm |
| Final extension P95 | 26.03% | 2.89% |
| Fresh minimum separation over accepted endpoints | 0.00270 mm | 0.13641 mm |
| Final intersection diagnostic | passed | passed |

Both use identical recorded world-space mouse targets. A separate directional
run checks fresh CPU contacts, bounds and planes at every step; its entire
trajectory is byte-identical. The directional CPU fallback reaches nine vertices
at most, and averages 0.00286 ms per step on this replay. It is sparse because the
collapsed faces open early, not because vertices or contacts were dropped.

A second 360-step run pulls 40 mm from the candidate's own attachment position
after settling. It completes pull, hold and release with a final minimum area
ratio of 0.739, a 3.92 mm maximum sewing gap and passing final collision checks.
Mean/P95 full-step time is 18.97/22.47 ms. The attachment continues moving after
release (218 mm net); it is not being held by a sleep or velocity gate. Gravity
and ongoing draping contribute to this motion, so it is not a measurement of
40 mm grab tracking accuracy. Endpoint total energy has no positive step within
the autonomous phases in either candidate run; that is a diagnostic of these
traces, not a general discrete dissipation theorem.

These single-run timings show comparable cost while the cloth is free to move;
the P95/max do **not** improve across the board. They do not establish constant
60 steps/s, and offline trace exports/audits are outside the step timer. The
final highpass RMS is 65.7 micrometres for matched controls and 58.5 micrometres
for natural controls: neither is zero, and different drapes make a jitter ratio
against the partly frozen baseline misleading.

The GPU tests are ignored by default and require an explicit invocation. They
check a collapsed patch's initial common tangential velocity, its exact return
to the ordinary path after opening, and bitwise unchanged supported motion
when no directional pair occurs. Existing free-fall, supported-cloth and
precision tests remain separate acceptance gates.

A stronger attempted test demanded exact constant center-of-mass velocity
through many steps of a severely compressed square. It exposed the old
isotropic finite-iteration path's lack of Galilean invariance after the first
step had opened the collapsed face. That broader limitation is not fixed here;
the test now distinguishes the directional step from the unchanged ordinary
path rather than claiming exact momentum conservation of the full solver.

This change permits motion that was previously prohibited. It cannot ensure
that a finite local solve finds every available direction, prevent all material
rank loss, guarantee an energy-monotone trajectory, or eliminate all jitter.
No sleep, velocity clamp, new damping or garment-specific exception is added.

### Validation record

- 161 CPU tests pass, including the four geometry-zoo tests with 512 seeded
  cases, 24 successive proposals and five samples per segment (61,440 sampled
  states). Fixed-avatar and reciprocal VF fixtures are included.
- Eight opt-in GPU tests pass: two directional/ordinary-path gates, four motion
  regressions (free fall, support, translated scene, topology growth), and two
  precision/portable-kernel regressions.
- A CPU-only 12-step replay of the private dress also releases all nine initial
  witnesses (28.5–36.9 mm net motion), ends with no face below 1% rest area,
  and passes the final intersection diagnostic. This verifies the CPU backend,
  not just the GPU's sparse fallback.
- The 33 desktop-feature library tests pass, including dependency-ordered
  sparse refinement matching sequential minimization exactly.
- The public unequal-length seam fixture completes all three 360-step headless
  replays. Pull displacement is 39.86 mm for a 40 mm target, followed by
  42.33 mm net motion after release. Its candidate CPU-oracle trajectory is
  identical and final intersection diagnostics pass. It does not activate the
  new close-pair path, so it is an ordinary-motion regression, not evidence of
  its speedup.
- The new isotropic replay trajectory is byte-identical to the archived
  previous-turn baseline. The user's original saved document and issue-project
  hashes are unchanged. No editor window was opened.
- CPU-only and desktop Clippy checks pass for all targets with warnings denied;
  formatting passes. The Release `cloth-fit` executable is rebuilt with the
  directional path enabled. No commit or push is part of this investigation.
