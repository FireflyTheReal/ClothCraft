# Solver consistency and convergence experiments

19 October 2026. These are headless CPU experiments on the saved dress. The
editor's VBD/OGC implementation and GPU kernels are unchanged. The new methods
must pass deformation, contact and interaction gates before a speed comparison
or production integration. No sleep, velocity threshold, increased damping,
changed rest geometry or private garment ID participates in the algorithms.

## What was wrong with the first prototypes

The original `xpbd` adapter remains available as a failing control. It clips
the inertial prediction against collision regions, then applies the simplified
sequential XPBD update. That simplification assumes the primal residual is
zero. Clipping the prediction invalidates the assumption even with linear
constraints; changing nonlinear gradients adds another source of error.
[XPBD, equations 8–18](https://mmacklin.com/xpbd.pdf) explicitly distinguishes
the full equations from this approximation. Increasing iterations of the
approximate update is therefore not a general consistency repair.

The original `avbd-finite` adapter is only the finite-stiffness branch of
[AVBD, sections 3.4–3.5](https://graphics.cs.utah.edu/research/projects/avbd/),
not its hard-constraint augmented-Lagrangian solver. Its residual-driven ramp
can leave some factors at 10% of target stiffness. Its additional isotropic
contact-Hessian approximation also penalizes tangential movement: a planar
frictionless contact with curvature `diag(0,k,0)` became `diag(k,k,k)`.
This changes convergence at a finite budget even though the gradient is exact.

## Two independent implementations

### Full primal residual (`xpbd-primal`)

This is a global Gauss–Newton/PCG method derived from the full compliant
equations, **not ordinary sequential XPBD** and not a new published algorithm.
For factored energies `U = sum(k*C²/2)`, eliminate the compliant forces
`lambda = -k*C`. With mass/drag diagonal `A` and the original inertial target
`y`, solve the actual stationarity equation:

```
gradient = A*(x-y) + J^T*K*C
H_GN     = A + J^T*K*J
H_GN * direction = -gradient
```

All material, sewing, grab and active contact factors participate together.
Contact factors use `sqrt(2*U_OGC)`; no contact law is replaced. PCG is matrix
free, with a diagonal preconditioner, 64 iterations maximum and relative
linear residual tolerance `1e-4`. Constraint Hessians are omitted from the
search matrix, not from the force or the energy checked by line search.

The initial iterate is the saved position, while `y` still contains inertia,
gravity and drag. Each proposed vertex displacement is limited along its
segment inside the original OGC ball and separating halfspaces. Individual
clipping can destroy global descent, so a tested descent fallback uses a
positive scalar preconditioned gradient at each vertex. Armijo backtracking
checks the full target objective for the current contact feature set.

### Finite AVBD with target completion and coupling (`avbd-target`)

This is an independent hybrid, not a reproduction of the authors' GPU AVBD.
It keeps finite material energies and the local AVBD curvature approximation,
but adds three explicit changes:

1. A stiffness floor reaches the full target during the first quarter of the
   sweep budget. The remaining sweeps solve the requested material.
2. Contact curvature is factored in the same manner as scalar material terms.
   Writing `q=sqrt(2U)`, keep `grad(q)*grad(q)^T` and approximate only
   `q*Hessian(q)` by its diagonal column norms. A quadratic planar contact
   retains zero tangential curvature; the exact contact gradient is unchanged.
3. Every fourth sweep, and on the last sweep, a full-target coupled primal
   step accelerates modes that propagate slowly through vertex-local updates.
   It uses the same energy acceptance and movement guards described above.

The analytic test of a patch driven toward a floor checks collision separation
and compares the final residual against the independent production energy
audit. The coupled linear test starts away from the inertial target and checks
the exact dense solution. Neither relies on a dress-specific configuration.

## Diagnostics and limits

`--convergence` records target residual RMS in newtons per free coordinate,
target objective, full-stiffness coverage, clipping and the energy difference
caused by refreshing contact features **at identical positions**. Final lab
residuals use fresh geometry before state commit. The production VBD metric
is assembled during local updates and is not an equivalent final residual;
the summarizer omits it from this comparison.

With convergence tracing, `refresh-NNNNN.json` captures the largest absolute
refresh jump in each physical step: dynamic positions and the 32 largest
per-feature energy changes. This is diagnostic geometry, not a resumable
checkpoint. It is emitted outside the editor and only when explicitly requested.

Descent for a frozen feature set does not guarantee monotonic energy after
a feature refresh. The first dress probe already exposes a sizeable change
under `xpbd-primal`; that is a contact-model consistency lead, not proof that
the production jitter has a single identified cause. A global energy
rejection guard could hide motion through sticking; none was added here.

The [OGC paper](https://graphics.cs.utah.edu/research/projects/ogc/) itself
discusses force discontinuities at open boundaries on the concave side of
faces (limitations, Figure 22a), and slow progress when conservative bounds
truncate large safe motions (Figure 22b). Those sections were read again in
the cached paper during this investigation. They motivate separating feature
transitions from bound-limited convergence. The measured dress jump still
needs a per-feature reproduction before assigning it to that specific case.

`--audit-every-step` checks original cloth surface intersections after every
accepted step, in addition to conservative movement regions. This is an
endpoint diagnostic under existing local seam exclusions, not exact
continuous collision certification. On failure, reports retain the accepted
state that failed the audit. A solver error leaves the caller's state unchanged.

## Other approaches reviewed in this pass

| Source actually consulted | Relevance and decision |
| --- | --- |
| [Small Steps in Physics Simulation](https://matthias-research.github.io/pages/publications/smallsteps.pdf), integration/contact sections | Test four smaller XPBD steps with one sweep each against four sweeps at the saved timestep. Recompute contacts every substep here: amortized detection is not a license to weaken collision coverage. This changes temporal integration, so it is not an equal-integrator speed comparison. |
| [Quasi-Newton methods for hyperelastic materials](https://users.cs.utah.edu/~ladislav/liu17towards/liu17towards.html), project abstract | Supports exploring coupled optimization and preconditioning. We implement Gauss–Newton/PCG, not their Projective Dynamics/L-BFGS solver. Curvature history across contact changes needs explicit handling. |
| [Newton VBD documentation](https://newton-physics.github.io/newton/latest/solvers/vbd.html), backend/contact sections | An external comparator combining cloth VBD and compliant ALM for rigid constraints. Its documented DAT contact path and material models must be mapped explicitly; no imported backend. |
| [ZOZO/PPF repository](https://github.com/st-tech/ppf-contact-solver), README and limitations | Candidate for an independent collision/strain quality reference. It describes itself as offline, with some interactive examples. No realtime claim on our dress; no wrapper implemented in this pass. |
| [B-spline cloth](https://simulation-intelligence.github.io/BS-Cloth/), project and evaluation description | Changes spatial discretization, bending and contact integration. Promising for mesh-quality work, but cannot isolate solver convergence on this fixed triangular benchmark. Not implemented. |

The AVBD project credits an author affiliated with Roblox. This does not
establish that this cloth implementation is the Roblox production engine.

## Reproduce

```powershell
# CPU only, two threads. Refuses an existing evidence directory.
.\scripts\solver-consistency.ps1 -Project path/to/dress.json `
    -Output output/consistency-new -Steps 120
python scripts/summarize-solver-consistency.py output/consistency-new --plot
```

The script archives executable, source files, input hash, compiler and CPU
provenance. It runs one-step convergence probes at 4/16/64 sweeps and matched
quiet/pull/release trajectories. Smaller-step XPBD multiplies all duration
and control counts by four. Each run preserves initial state, portable input,
protocol, metrics and final state. These unoptimized prototypes are evaluated
for quality first. Read [the measured results](solver-consistency-results.md).
