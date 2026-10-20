# Dress solver consistency: results

19 October 2026. This pass tests correctness and convergence before speed.
Methods and sources are described in [solver-consistency.md](solver-consistency.md).
The existing editor solver remains the control. None of the experimental
methods qualifies as a replacement on the saved dress at the tested budget.

## Protocol

The source dress is unchanged: original file SHA-256
`E542E066F4BF662149F8603BAB47913A60498915689386F0BD51503E4D7AACCB`.
The frozen input comes from the preceding laboratory's portable snapshot.
Each run starts from the same 5,126 dynamic vertices, 9,546 cloth faces,
six panels, seven seams, materials, velocities and avatar. No remeshing.
The initial state is not a certified equilibrium; one panel is disconnected.

Single-step probes use 4, 16 and 64 nonlinear sweeps, extended to 200 for the
two coupled variants. Trajectories use four
sweeps per 1/60 s step, for two seconds. Interaction starts after 0.5 s,
pulls the same barycentric grab 40 mm in world Z over 0.5 s, holds for 0.25 s,
then releases for 0.75 s. The smaller-step XPBD control instead uses one sweep
per 1/240 s step, scaling every control count by four. This changes integration.

All runs are CPU-only with two Rayon threads. Per-sweep tracing is enabled
only for convergence probes. Every accepted step is checked for endpoint
surface intersections with the existing local sewing exclusions, in addition
to OGC movement bounds. These checks are not a continuous/exact certificate.

## Findings

The linear coupled test recovers the dense analytic solution from a deliberately
displaced initial iterate. A floor-impact test compares the new residual with
the independent production energy audit. The original isotropic contact
Hessian stalls tangential convergence. Preserving contact direction helps;
adding coupled polishing reduces the test's residual to approximately
`6.7e-15 N` at 64 sweeps. This validates that bounded problem, not the dress.

On the dress's first physical step, the fresh final residuals are:

| Research adapter | 4 sweeps | 16 sweeps | 64 sweeps | 200 sweeps |
| --- | ---: | ---: | ---: | ---: |
| Original clipped XPBD | 6.29 N | 22.03 N | 18.91 N | Not run |
| Original finite AVBD | 0.65 N | 0.52 N | 0.38 N | Not run |
| Full primal residual / PCG | 14.56 N | 4.01 N | 3.25 N | 1.44 N |
| Target-complete finite AVBD hybrid | 0.54 N | 0.50 N | 0.41 N | 0.52 N |

More iterations of the original XPBD do not repair its inconsistent primal
assumption. The coupled method is consistent with the full equations but
converges poorly on this contact-rich geometry. Target completion alone is
also insufficient: local convergence, changing contact features and motion
bounds remain significant. The original finite AVBD still has factors below
target stiffness, so a low residual of its softened surrogate is not enough.

The target-complete variant's ramp occupies a quarter of the configured
budget: its rows are quality-versus-budget tests, not nested prefixes of one
fixed continuation trajectory. Even at 200 sweeps, neither candidate reaches
a small final equilibrium residual on this step. The two extended probes
also pass intersection checks, bringing this pass to **1,694 accepted steps
across 22 runs**.

The CPU VBD production metric is not listed in this residual table: it is
assembled during local updates rather than recomputed at the complete final
state. Shared seam/stretch diagnostics below remain comparable.

## Complete quiet and pull/release trajectories

| Adapter | Final seam gap, quiet / release | Final stretch P95, quiet / release | Peak grab response versus quiet control |
| --- | ---: | ---: | ---: |
| Production CPU VBD | 13.57 / 13.46 mm | 18.96 / 19.09% | 31.51 mm |
| Full primal residual / PCG | 66.66 / 72.00 mm | 25.79 / 25.72% | 19.43 mm |
| Target-complete finite AVBD hybrid | 67.38 / 72.07 mm | 62.12 / 62.51% | 9.59 mm |
| Smaller-step clipped XPBD | 742.46 / 692.80 mm | 855.06 / 849.08% | 202.69 mm |

Every one of these eight runs completes its requested two physical seconds
and passes the per-step/final intersection checks. There are 1,680 accepted
trajectory steps (the smaller-step pair accounts for 960), plus 12 accepted
single-step probes. All initial-state files have SHA-256
`18174fe9d1afdab39b95bd9dde0826b5e02874319f51776a1bbce179976f87df`.
The smallest audited separation in this batch is about `3.49e-10 m`:
positive under the existing f64 checks, not a comfortable geometric margin.

The variants still fail material/sewing quality at this budget. The hybrid's
small grab response is also a warning against calling it fluid merely because
its motion is limited. Response is measured as grab-point displacement relative
to the matched no-input run, not as compliance or a calibrated stiffness.
After release, the final response differences are 1.27 mm for VBD, 5.34 mm for
the coupled primal method, 1.35 mm for the AVBD hybrid and 179.68 mm for the
small-step hybrid. A small difference alone is not a settling or fluidity pass.

The short release lasts 0.75 s. These runs do not establish long-term settling,
absence of jitter, friction quality or production performance. The very large
XPBD deformation is an explicit failed result, not a faster cloth solver.

## Contact-refresh lead

The full-residual probe records an approximately **+0.62 J** contact-energy
change at a refresh before its fifth sweep, with positions held identical
during that comparison. This means a decrease in the frozen-feature objective
does not establish descent of the refreshed objective. OGC's paper discusses
related force discontinuities on concave open boundaries, but this specific
event has not yet been classified as that geometric case. It is not proof
that every production jitter event has the same cause.

The final diagnostic capture localizes the dominant change to one cloth
edge–edge feature: **+0.619052822 J**, with only a roughly `-1.84e-7 J`
point–face change alongside it. The total is **+0.619052638 J**. Geometry
and feature IDs are retained in the private `refresh-00000.json`; no ID is
used by the solver. This event requires an edge-feature transition analysis;
the paper's concave-face example is context, not its established explanation.
The final instrumented executable reproduces the first 64 sweep diagnostics
of the earlier executable exactly in its 200-sweep probe.

The lab can now capture the geometry and largest changing contact terms for
such events. No production energy guard, friction, sleep or numerical damping
was introduced to hide the issue.

## Next experiments

1. Reduce the captured feature transition to a public synthetic crease fixture.
   Audit feature identity, deduplication, force activation and energy at the
   same geometry before changing the model.
2. Test a coupled solve that accounts for active movement-region constraints
   in the linear subproblem. Clipping a global unconstrained direction can
   discard much of its benefit; increasing PCG work alone is not a remedy.
3. Keep full target stiffness and independent force audits. Require comparable
   seams/stretch plus longer pull/release and settling windows before porting
   a candidate to GPU or calling it faster.

Private raw evidence and archived sources/executables are under
`output/solver-consistency-final-20261019/`; initial ablations are retained in
`output/solver-consistency-20261019/`. Public numeric evidence is recorded in
[solver-consistency-validation.json](solver-consistency-validation.json).

Validation: **173 CPU tests**, rustfmt, Clippy across all CPU targets with
warnings denied, and six template validations pass. The quiet and pull VBD
controls match the previous campaign's final state files byte for byte,
including velocities and time. The user's original save is unchanged.

To repeat the additional probe after the scripted matrix:

```powershell
output/consistency-new/solver_compare.exe --project output/consistency-new/single-xpbd-4/source.json `
  --output output/consistency-new/single-xpbd-primal-200 --backend xpbd-primal `
  --iterations 200 --steps 1 --convergence --audit-every-step
# Repeat with --backend avbd-target and a new output directory.
```
