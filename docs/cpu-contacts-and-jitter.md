# CPU contacts and Garment motion diagnosis

18 October 2026. CPU only, two workers. This pass speeds up edge contact
geometry and adds offline force diagnostics. It does **not** fix jitter by
changing damping, prediction, constitutive laws or the accepted positions.

## Implemented

`ogc/simd.rs` evaluates four independent edge pairs with AVX **f64** arithmetic.
It computes the segment parameters, closest points and distance. Runtime
detection checks CPU/OS support once per scan; unsupported processors use the
existing scalar routine. `OgcWorld.cpu_kernel = CpuContactKernel::Scalar` selects
the oracle for diagnostics. This is a CPU implementation choice, not another
physical solver or a new saved-project setting.

Both the cached CPU broadphase and the primitive-candidate entry point used by
the hybrid backend feed the same narrow phase. Only complete four-pair packets
use AVX; the tail stays scalar. Vertex/face closest features, feasible regions,
canonicalization, incidence and the potential derivatives remain unchanged.
There are no approximate reciprocals or fused multiply-adds. The dot-product
ordering, degeneracy/parallel thresholds and signed-zero clamp behavior match
the scalar implementation.

Distances are recomputed from current coordinates for **every** retained pair.
Pairs still constrain movement even when their feasible-region force is rejected.
The candidate envelope, local seam exclusions, zero-energy nearby terms and OGC
ball radii are unchanged. This does not implement contact recycling or reuse
old distances/normals.

`vbd/audit.rs::audit_vertex_forces` evaluates the physical force at selected
vertices using the supplied contacts. It includes material, sewing, contact,
gravity and an optional grab, but excludes inertia and mass-proportional drag.
It is read-only, opt-in and never called by the interactive worker.

`examples/contact_motion_probe.rs` checks an archived trace against the saved
project's exact initial state, ranks motion hotspots, then alternates scalar and
SIMD detection order at each state. It compares contact identities, coefficients,
incidence, candidate counts, minimum distance and all movement radii. Reports
include active contact changes, force changes at a fixed position when features
are refreshed, sewing forces, fresh endpoint force residuals and seam energies.
Hotspots use panel IDs and material coordinates; no persistent mesh indices.
The older `contact_probe` derivative checker is a separate unchanged tool.

## Measurement

The owner's saved Garment matches the initial state of the archived rollback
trace. It has 6,731 vertices; 361 states span 360 accepted steps at 1/60 s.
Each replay measures the complete **CPU detector**, including broadphase/cache,
exact geometry, canonicalization and incidence. It uses a 12 mm query radius.
The first 60 steps are excluded from timing aggregates. Two final interleaved
runs on the same workstation give:

| Detector time | Scalar | AVX | Reduction |
|---|---:|---:|---:|
| Run 1, mean | 25.22 ms | 22.54 ms | 10.6% |
| Run 1, P95 | 28.94 ms | 26.03 ms | 10.1% |
| Run 2, mean | 25.76 ms | 23.24 ms | 9.8% |
| Run 2, P95 | 30.62 ms | 28.02 ms | 8.5% |

Geometry processing alone drops from 22.06 to 19.41 ms in run 1 and from
22.54 to 20.01 ms in run 2. Every compared contact set and movement bound agrees
bit for bit. These CPU detector costs include work that the interactive hybrid
path offloads to GPU. They must **not** be compared directly with its earlier
3.57 ms CPU geometry/contact stage, nor advertised as a GPU or full-step speedup.
No new 60 steps/s claim is made here. Detailed evidence belongs in
[the validation record](cpu-contacts-and-jitter-validation.json).

## What the jitter probe found

The largest highpass-motion values in this six-second recording are around
`seam_5`, joining `panel_3/edge_1` to `front/neck-bottom`. Their rest lengths are
90 and 80 mm: **12.5% easing**, which is allowed and is not itself an error.
The seam's peak gap after the warmup is about 0.425 mm. Stored sewing energy is
legitimate; reducing it to zero is not the goal.

The twelve highest-amplitude points measure 0.164–0.204 mm RMS of
`(v_new-v_old) * dt / 2`. This highpass quantity also detects physical vibration
and is not a proof of numerical jitter. Six of those points participate directly
in `seam_5`. One endpoint on `panel_3` has **no active contact at any of the 300
measured states**, while carrying about 0.078 N mean sewing-force magnitude and
showing 0.175 mm highpass RMS. Its minimum fresh movement radius is 1.356 mm.
The other selected hotspots also have millimetre-scale bounds, not the tiny
precision-limited balls behind the earlier stall.

Nearby points do experience contact changes: the leading hotspot changes active
contact keys in 31% of measured steps and its largest fresh-vs-frozen contact
force difference is about 0.112 N. Active-key churn includes activation at the
shell as well as changes of feature identity. These are correlations; neither
the absence of a local contact nor contact churn establishes the origin of a
wave propagating through the cloth.

This narrows the investigation toward **coupled seam/material convergence**,
while keeping contact-force consistency in scope. It does not justify another
global prediction guard, sleeping, radius relaxation or velocity smoothing.
The earlier [energy audit](energy-and-release.md) still finds a net energy drop
over the post-warmup interval despite individual rises; perpetual energy
generation has not been established.

Fresh endpoint residuals are not the actual residuals of the GPU solve: its
contact set and movement centres were formed earlier in the step. Similarly,
fresh balls do not reveal which local updates were clipped. The next causal
experiment should instrument those actual sets and compare unchanged seam laws
with a coupled seam solve, under the **same** pull target and release history.
No coupled cloth solver or new damping law was enabled in this pass.

## Reproduce

```powershell
$env:CARGO_INCREMENTAL = '0'
$env:RAYON_NUM_THREADS = '2'
cargo build --release --locked --no-default-features -j 2 --example contact_motion_probe
.\target\release\examples\contact_motion_probe.exe --project path/to/garment.clothcraft.json --trace path/to/trajectory.jsonl --steps 360 --output output/contact-motion
python scripts/summarize-contact-motion.py output/contact-motion --output output/contact-motion/analysis.json
.\scripts\check-cpu.ps1
```

Private geometry, raw traces, per-hotspot reports and timings stay under ignored
`output/cpu-contact-probe/`. The archived input is validated before analysis;
exporting a different drape requires a matching new trace.

Regression coverage includes 40,000 edge pairs with degenerate, parallel,
endpoint and short-edge cases; moving cached/direct contact comparisons;
and 180 complete CPU steps of unequal sewing, pulling, holding and releasing.
The latter compares positions and velocities bit for bit and checks that the
released cloth moves. Force diagnostics are checked against finite differences
of the complete potential and the analytic free-fall force. This is numerical
regression coverage, not a substitute for another interactive GPU assessment.

`scripts/check-cpu.ps1` passes: formatting, **148 CPU tests**, all-target CPU
Clippy with warnings denied, and all six template validations. The ordinary
suite still requires no GPU.

The saved document and desktop executable are unchanged. No GPU execution,
window launch, commit or push was performed.
