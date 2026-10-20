# Wire contact assembly: snapshot audit and rejected forest correction

Status, 20 October 2026: CPU research only. No force law is installed, and no
new simulation trajectory is generated. The editor, production VBD/OGC,
separation bounds, prediction and damping are unchanged.

## What is evaluated

The [two-element kernels](contact-patch.md) do not define a whole-mesh
discretization. `OgcWorld::audit_wire_quadrature` now evaluates the wire-density
construction from [Convergent IPC, sections 4.3–4.4](https://arxiv.org/html/2307.15908):
sum finite edge–edge barriers, then subtract `(degree - 1)` times each
vertex–source-edge barrier. The degree belongs to the allowed target graph
after applying the current source edge's explicit seam exclusions.

The paper's positivity argument depends on a local activation scale. We also
count vertices in connected components of the source-edge capsule intersected
with the target wire graph. This is a diagnostic analogue of the local-scale
condition, not a universal surface-contact certificate.

Values are **per-source-edge energy proxies in joules** using our current
activation. Area quadrature, vertex–face energy and the complete paper solver
are absent. The audit includes both moving-to-fixed and fixed-to-moving
queries and omits fixed-to-fixed terms. It builds fresh BVHs and never mutates
the simulation's caches.

The most adverse 32 densities per radius are checked by a complete edge scan
and globally computed allowed degrees. This oracle shares the finite-distance
kernel; it independently checks search and assembly, not geometry derivatives.

## Dress evidence

Inputs are the frozen 5,126-vertex dress checkpoint at 49.45 s and the saved
endpoint at 50.1167 s of the earlier 40-step production-control pull/release.
Neither state is a certified equilibrium. The existing pull/release trajectory
is reused; this pass performs no stepping.

| Contact radius | Checkpoint: active sources / negative densities | After release: active sources / negative densities |
| --- | ---: | ---: |
| 3 mm (current setting) | 845 / 0 | 771 / 0 |
| 1.5 mm | 2 / 0 | 0 / 0 |
| 0.75 mm | 2 / 0 | 0 / 0 |
| 0.375 mm | 2 / 0 | 0 / 0 |
| 6 mm (stress audit) | 8,123 / 5 | 7,724 / 6 |
| 12 mm (stress audit) | 13,961 / 16 | 13,355 / 19 |

At 3 mm, every active component has at most one included target vertex. At
6 mm the maximum becomes 23 and 20; at 12 mm it becomes 47 and 44. Minimum
signed densities are respectively -1.871 / -1.700 J at 6 mm and
-100.394 / -84.654 J at 12 mm.

All 198 checked source/radius combinations match the assembly oracle; active
pair counts match exactly and maximum energy discrepancy is 7.68e-13 J.
The larger radii are diagnostic parameter sweeps on unchanged positions,
not new cloth simulations. Passing at 3 mm is encouraging but establishes
neither future validity under deformation nor smooth forces.

Simply reducing the radius is unsuitable as an automatic fix: it removes
almost the entire measured force support in these snapshots.

## Generic counterexamples

`tests/wire_quadrature.rs` preserves independent rejection cases:

- A tiny tetrahedral target wire graph inside the activation neighborhood
  over-subtracts. The measured normal force is **-446.62 N**, toward the
  target. This demonstrates attraction, not merely a different energy zero.
- Applying the existing EE parallel mollifier only to positive terms also
  breaks positivity on a two-edge patch. The test covers angles from exact
  parallelism through 0.75 degrees, plus a nonfailing 1.5-degree control.
  This deliberately inconsistent mixture is not production OGC.
- An independent full-scan oracle agrees after seam exclusions remove edges.

The audit also reports that deliberately mixed mollifier construction. Its
reference lengths come from the original checkpoint world, even when a later
state is inspected, matching a continuous replay rather than a cold restart.
No material rest geometry is changed.

## Independent forest experiment: positive but rejected for force continuity

We tried subtracting only when a vertex event merges distinct edge-contact
components, in increasing distance order. Cycle closures no longer subtract.
This is an independently assembled graph experiment, not a claimed novel
algorithm or implementation of the paper's complete method.

For a forest of merges, root each tree at its largest primitive barrier. Every
other node contributes its own barrier minus its parent-merge barrier, which
cannot be larger. The density is therefore at least the maximum primitive
barrier. This also explains why the original correction works on local
acyclic graphs.

The forest version passes that lower bound on every audited finite source,
including the 6 and 12 mm dress stress cases. It changes the tetrahedron's
normal force to +226.69 N. **It still fails a necessary promotion gate.**

A six-vertex public fixture swaps two equally distant merge events. As the
finite-difference interval shrinks from 1e-7 to 1e-9 m, one-sided energy slopes
converge to approximately +111.248 and -111.248 N. The resulting force jump is
about **222.497 N**. The original signed formula's slope difference tends to
zero in that fixture.

Thus skipping cycles repairs positivity but creates a nonsmooth force when
the chosen forest changes. Do not install it as a jitter fix or freeze its
merge choices during line search. See the
[smooth averaging proposal](smooth-forest-contact-proposal.md) for an
unimplemented follow-up, with its own unresolved gates.

## Reproduction and evidence

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test wire_quadrature -- --test-threads=2 --nocapture
cargo build --release --locked --no-default-features -j 2 --example wire_quadrature_probe
.\target\release\examples\wire_quadrature_probe.exe --project PATH_TO_FROZEN_PROJECT --output output/new-wire-audit.json --radii-mm 3,1.5,0.75,0.375,6,12
```

Use `--state PATH_TO_STATE_JSON` to audit a matching later snapshot without
changing the checkpoint world. A fresh output filename is required.
Raw evidence lives in ignored `output/wire-quadrature-20261020`:
`forest-checkpoint.json`, `forest-after-release.json`, `tests.log`, and
`regression-tests.log`. Audit timings include diagnostic work and are not
simulation-step or frame-rate measurements. No GPU or desktop window was used.

Validation: 29 targeted regression tests, including five new
audit/counterexample tests, pass; CPU and desktop all-target Clippy pass with warnings denied.
Formatting and tracked-diff whitespace checks pass. Run
`python scripts/summarize-wire-quadrature.py output/wire-quadrature-20261020`
to regenerate the aggregate evidence without private geometry or feature IDs.
See [validation data](wire-quadrature-validation.json).
