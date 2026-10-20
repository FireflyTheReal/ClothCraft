# Draped dress checkpoint and residual motion

19 October 2026. The nine-vertex directional release remains enabled. This pass
does **not** change production forces, collision coverage, damping, prediction or
iteration defaults. Remaining jitter is not fixed.

## Reusable benchmark

`scripts/drape-checkpoint.ps1 -Project <document> -Gpu -Compare` builds a headless
runner, exports a portable source copy, runs 1,800 steps, saves positions,
velocities, physical time and settings, then compares three rest continuations.
GPU use is opt-in; two CPU workers. A new evidence directory is mandatory.
`scripts/summarize-drape.py <directory>` separates seam-connected components,
reports full-step costs and keeps the distinction between drift and oscillation.
It reads OBJ groups only from the matching compiled snapshot. It never writes
vertex indices into an editable document.

The owner's current six-panel, 5,126-vertex dress is preserved locally under
`output/dress-settling-20261019/source/`. The useful checkpoint is
`output/dress-settling-20261019/drape-30s/fitting-project.json`, with its adjacent
assets. It contains 30 more physical seconds than the source, ending at 79.45 s.
No source document was overwritten. Private geometry and trajectories stay in
ignored output; manifests and summaries there identify the exact inputs/binary.

**This is a draped checkpoint, not a certified equilibrium.** Prediction history,
contact caches and motion-monitor history are rebuilt on loading. The first
restart steps can be transient. Comparisons start from identical x/v/settings,
and the timing table below excludes the first five seconds of each continuation.
Do not zero velocities or claim bit-identical continuation of an uninterrupted
solver. A future research checkpoint should also carry validated predictor state.

## Findings on this scene

Five panels form the sewn dress; the sixth has no seam connection and falls.
That piece alone moves about 2.04 m down during the 10 s baseline continuation.
It is retained in every run. Global speed and global gravitational energy cannot
be interpreted as the dress's equilibrium diagnostics. At the 30 s checkpoint,
the sewn component's instantaneous unweighted RMS speed is 3.80 mm/s, while the
independent piece is at 212.46 mm/s. Even the sewn component is still moving.

RTX 3090, hybrid GPU VBD/OGC, 16 default iterations, dt=1/60 s, unchanged geometry,
materials and collision parameters. These are sequential single-run comparisons,
not confidence intervals or render FPS. Costs include full solver steps but not
file export/offline audits.

| Run | Last 5 s mean step | P95 step | Cost per 1/60 s | Global high-pass RMS mean |
|---|---:|---:|---:|---:|
| End of 30 s drape | 9.65 ms | 10.65 ms | 9.65 ms | 10.87 µm |
| Restart, 16 iterations | 11.31 ms | 12.27 ms | 11.31 ms | 34.95 µm |
| Restart, 64 iterations | 19.25 ms | 22.05 ms | 19.25 ms | 22.52 µm |
| Restart, 2 × dt/2, 8 iterations each | 7.91 ms | 9.00 ms | 15.82 ms | 68.08 µm |

More iterations reduce the same-dt oscillation indicator by 36%, but exceed the
60 Hz budget. Smaller substeps increase both this indicator and final sewn-piece
speed in this experiment. The high-pass measure contains dt and real acceleration;
it is not a frequency-independent jitter certificate. Neither trial is promoted.

A separate 360-step pull/hold/release starts at the checkpoint and pulls a material
point on the ruffle by 40 mm. Full-step mean after warmup is 17.71 ms, P95 20.79 ms:
the quiet timing does not imply 60 Hz during interaction. Fresh final intersection
validation passes. A second replay with exact same world-space controls checks
every GPU contact set against fresh CPU detection; its trajectory hash is identical
(`757eab9f8237d0adaa60530f0a47564d3321ea353a4a7b31b1d11844b4bfd175`).
Existing seam exclusions and floating-point limitations still apply.

## Energy evidence and next experiments

An offline f64 audit of the first 110 autonomous restart steps finds 31 positive
mechanical-energy changes (accounting identity error <=4.45e-16 J). It separates
fixed-feature dynamics from contact refresh at the same position. Two different
effects are visible:

- At step 63, fixed-feature energy decreases by 0.161 mJ, but changing the contact
  representation adds 6.152 mJ to the measured potential.
- At step 80, fixed-feature energy increases by 0.372 mJ while the refresh change
  is negligible. Residual work is +0.780 mJ; drag, implicit kinetic loss and the
  nonlinear remainder account for the difference. This residual includes movement
  bound reactions and incomplete convergence, not an isolated force bug.

The initial four restart steps also have a larger transient. Full-scene energy
includes the falling piece. These observations justify separate investigations,
not an energy clamp. Priorities:

1. Reproduce uninterrupted vs cold vs predictor-restored checkpoint continuations.
2. Audit contact-feature transitions with forces on both sides at identical
   positions. Keep live collision detection fresh; freezing a feature set is only
   an offline measurement technique.
3. Attribute final residual work to connected material patches and bound activity;
   compare f64 and GPU energy differences on the same accepted iterates. The GPU's
   absolute local acceptance tolerance remains a hypothesis, not an identified cause.
4. Evaluate complete coupled material/seam patches only if the residuals identify
   them. Repeat pull/release and sliding before accepting a change.

## Research consulted

- [Divide and Truncate, 2026](https://arxiv.org/html/2604.15513v1), especially
  limitations: directional collision feasibility does not establish conservation
  or eliminate discontinuities inherited from the contact energy. This supports
  retaining the released motion while diagnosing forces separately.
- [Geometric Contact Potential](https://arxiv.org/html/2402.00719v3): smoothness and
  spurious mesh-dependent contact forces are relevant. This is not a ready-made
  realtime backend, nor a promise to remove every tangential discretization artifact.
- [Implementing VBD Damping Properly](https://ankachan.github.io/posts/2026/03/implementing-vbd-properly/):
  local diagonal stiffness times absolute velocity can resist free fall. Any
  future internal damping needs full relative strain-rate coupling and invariance
  tests; no extra damping was added here.
- [Small Steps](https://mmacklin.com/smallsteps.pdf) motivates the substep experiment,
  but its XPBD results do not predict this VBD implementation's cost or quality.

The useful separation is collision safety, convergence of forces, and material
dissipation. Improving one does not prove the other two.

Follow-up: the [synthetic CPU settling zoo](settling-zoo.md) isolates a cold
prediction disturbance and exhausted contact searches on small public fixtures.
It also finds force-balanced resting eased seams and force-imbalanced false
rests. These findings motivate specific dress experiments; they do not certify
a fix for this GPU checkpoint.
