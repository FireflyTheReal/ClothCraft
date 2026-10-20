# Contact refresh: distinguishing edge and face transitions

The editor still uses its existing VBD + OGC model. The interior-edge
experiment remains disabled by default. No damping, sleeping, material,
sewing or collision-region changes were promoted in this investigation.
See [the edge experiment and pull/release results](edge-transition-results.md).

## Production VBD capture

The earlier 0.619 J edge event was captured in the full-primal research
solver. It must not be presented as the cause of every VBD oscillation.
Production CPU VBD now has the same passive, opt-in detailed refresh capture:
old detection positions, current positions and the largest per-feature
energy changes, evaluated at **identical current geometry**.

With `solver_compare --backend vbd --convergence`, each step writes
`vbd-refresh-NNNNN.json` when a refresh occurs. The event retained is the
largest signed change within that step: positive changes take priority over
energy removal. Scalar changes for every refresh remain in `convergence.jsonl`.
The capture does not include a resumable velocity/predictor checkpoint.

A six-step replay of the frozen dress, at 1/60 s and 16 iterations, identifies
the previously observed +0.088282421 J event:

| Contribution | Change at refresh |
| --- | ---: |
| Dominant point–face term | +0.088282070 J |
| Another point–face term | +0.000000351 J |

The dominant point's plane-projection barycentric coordinate crosses from
−0.00661985 to +0.00260359. Its plane gap changes from 1.5811 to 1.6712 mm.
Thus a face contribution appears with finite energy despite the gap
increasing. This is a feature-selection event, not work performed by moving
the vertices during the refresh. It does not by itself prove sustained
mechanical-energy growth or identify every source of jitter.

A 65-sample interpolation of the captured detection geometries confirms the
sequence: the initial closest edge fails its feasibility test; at sample
46/64 the closest feature becomes a face and contributes 0.091787575 J.
Its four-vertex gradient norm is 175.406 N. At the final sample the contribution
is 0.088282070 J. All sampled geometries pass the intersection diagnostic.
This is a sampled transition study, not a continuous-collision proof.

All six endpoint intersection audits pass. The final serialized state is
byte-identical to the prior diagnostic control. This checks passivity for
that replay; diagnostic timing is not suitable for performance comparisons.
Private geometry and vertex identities remain in ignored
`output/solver-research-20261020/vbd-capture`.

## Independent public fixture

`tests/edge_transition.rs::concave_face_entry_is_a_separate_refresh_discontinuity`
uses seven vertices and three triangles. Two incident faces form a valley;
the third supplies the moving point. Crossing a face boundary by two
nanometres adds over 0.5 J to a point–face term at positive separation. The
shared edge fails its neighboring-face feasibility test.
The standalone probe measures an activation of 0.565853180 J and a
four-vertex gradient norm of approximately 1,378 N.

The test verifies separated geometry, the missing/added feature, the passive
capture's old/current coordinates, and unchanged movement bounds under the
edge experiment. The point–face contribution is exactly unchanged by that
experiment. This is a generic fixture, not a special case for a garment ID.

## Decision and next experiment

Smoothing EE alone cannot remove face-family switching. A complete candidate
must address feature transitions together, keep adequate contact support,
and pass the existing matched dress and sliding/pull/release gates. Keeping
collision bounds conservative is necessary but does not certify a faithful
contact force law.

An exploratory diagonal-preconditioned L-BFGS variant was also rejected before
being added as a backend: on the floor impact convergence gate, increasing
from four to 64 iterations increased the freshly audited residual from
0.005449 to 0.019999 N. Its two-loop implementation and failing log are archived under ignored
`output/solver-research-20261020`; it is not available in the editor or CLI.
Changing optimizer families is not evidence that feature switching is fixed.

## Reproduce

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test edge_transition -- --test-threads=2
cargo build --release --locked --no-default-features -j 2 --example solver_compare
target/release/examples/solver_compare.exe `
  --project path/to/frozen/source.json --output output/new-vbd-capture `
  --backend vbd --steps 6 --iterations 16 --convergence --audit-every-step

# Independent public PF fixture, then the actual captured dress transition.
cargo build --release --locked --no-default-features -j 2 --example edge_transition_probe
target/release/examples/edge_transition_probe.exe `
  --point-face --output output/new-synthetic-face.json
target/release/examples/edge_transition_probe.exe `
  --point-face --project path/to/frozen/source.json `
  --capture output/new-vbd-capture/vbd-refresh-00003.json `
  --output output/new-dress-face.json
```

The dress input used here is the portable
`output/solver-consistency-final-20261019/single-xpbd-4/source.json`.
No GPU workload, editor launch, executable replacement, commit or push was
part of this follow-up.

Validation: 30 focused CPU tests pass, including the production sliding and
pull/release regression; all-target CPU Clippy passes with warnings denied,
and the desktop targets compile. The updated probe also passes its targeted
Clippy check and release build. Both PF probes check identical OGC radii,
directional bounds and minimum separation with the EE experiment on/off.
