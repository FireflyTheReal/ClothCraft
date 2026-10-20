# Editor and collaboration snapshot validation

21 October 2026. Production contact laws, solver defaults and material laws are unchanged by the editing pass. The brush is a new temporary external control.

## CPU and UI

The CPU suite passes 283 tests. The desktop headless UI target passes 44 tests, with one optional image-export test ignored. CPU and desktop all-target Clippy pass with warnings denied.

The optimized desktop build passes the native authoring scenario: 55 pointer
actions covering drawing, sewing, selection, rotation, materials, undo and
save/reload. The native Grab scenario picks the cloth and accepts grabbed
simulation steps; the maximum vertex displacement is 19.10 mm over 0.0833 s
of simulated time. Its harness holds the pointer until an accepted step,
with a wall-clock timeout, so GPU startup cannot silently consume the gesture.
The native checks use public synthetic scenes, not the owner's saved garment.

Reproduce these opt-in window checks after building the release executable:

```powershell
cargo build --release --locked -j 2 --bin cloth-fit
.\target\release\cloth-fit.exe --steps 0 --threads 2 --ui-authoring-smoke --output output/editor-authoring
.\target\release\cloth-fit.exe --steps 0 --threads 2 --ui-interaction-smoke --output output/editor-grab
```

Local logs and captures are under `output/editor-release-20261021` (ignored
by Git). An incremental-cache compiler panic during validation was bypassed
with `CARGO_INCREMENTAL=0`; the checks then completed. Release builds succeeded.

## Brush and contact

The brush CPU tests check normalized force, energy derivatives, rigidly
translated targets, component membership, invalid-input rejection and
pulling into a surface followed by release. The optional GPU gate checks
forces and contact search at identical geometry on every step, plus separate
CPU/GPU separation and motion gates over a 36-step pull/hold/release.

Do not interpret this as CPU/GPU trajectory equivalence. A proposed 0.5 mm
pose-agreement gate failed even at 128 iterations. The existing point-only
control diverges by up to 34.05 mm over this particular protocol; the brush
case differs by up to 43.28 mm. The production CPU path can refresh contacts
and movement bounds between sweeps, whereas the GPU path retains the initial
set throughout a step; their Hessians and line searches also differ. The
brush run performs 51 CPU detections versus 36 GPU detections. Increasing the
iteration count alone does not make these methods identical.

The invalid pose-equivalence gate was replaced with the force, coverage,
separation and motion checks above, not a larger pose tolerance. Maximum
same-geometry force discrepancy on the tested GPU is 1.42e-5 N. This small
fixture does not establish draped-garment convergence, freedom from jitter,
or a new performance claim. GPU execution remains explicitly opt-in:

```powershell
cargo test --locked -j 2 --test gpu_grab_brush -- --ignored --test-threads=1 --nocapture
```
