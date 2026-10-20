# Contact oscillation — 14 October 2026

This is a historical correction and measurement record. It does not claim that current residual jitter is solved; see [the later rollback](settled-jitter.md) and [contact-refresh diagnosis](contact-refresh-follow-up.md).

## Diagnosis and correction

Reported shoulder jitter exists in accepted solver positions. On a draped T-shirt, increasing iterations from 16 to 64 reduced high-frequency displacement P95 only from 0.701 to 0.564 mm. A horizontal piece starting 50 µm above a plane's contact zone reproduced 1.35 mm normal jumps. Neither reproduction depends on rendering.

Detection included only interactions already closer than radius `r`. Prediction or an iteration could enter that region during a step without the corresponding potential. The next step reintroduced strong repulsion; corrections became velocity. OGC bounds could prevent crossing without preventing oscillation within the offset layer.

`ogc/world.rs` now retains nearby feasible interactions up to query radius `rq`, including zero-energy terms. **Physical activation remains `r = 3 mm`**: no preventive long-range force, new thickness or extra damping. Coloring includes dormant terms. Potential and derivatives are reevaluated during solving/local search; centers, distances and conservative radii remain those of detection. Canonical sorting/deduplication replaces per-contact tree insertion; T-shirt trajectories before/after this optimization are bitwise identical.

The [OGC paper](https://graphics.cs.utah.edu/research/projects/ogc/Offset_Geometric_Contact-SIGGRAPH2025.pdf), Algorithms 1–3 and §4.3, was reread: selection at `r` accompanies refreshing sets during resolution. Our hybrid backend detects only once per step. Retaining terms to `rq` is an independent adaptation to that schedule, not a literal reproduction. Primitive types and feasible regions are still selected at detection; this extension does not guarantee complete force coverage under large movement or feature changes.

GPU computation now uses a frame translated to the garment center, calculated in f64 each step. Positions, targets, bounds and mouse grabs share that frame; readback restores world coordinates. Membrane/sewing use position differences to reduce f32 cancellation. Derivatives use the same coefficients as those expressions. Rest geometry, material weights, gravity, adaptive initialization and 16 iterations remain unchanged. This is not a display filter.

`VbdMetrics.contacts` counts positive-energy terms at detection; `contact_terms` counts all retained interactions. `ContactSet.contacts` now includes dormant terms, so its size is not an active-collision count. Dormant terms have a cost; the GPU kernel exits their evaluation before derivatives when distance exceeds `r`.

## Measurements

RTX 3090 / Vulkan, two CPU threads, dt 1/60 s, 16 iterations, identical starting snapshots in each comparison. Three-second traces, first second excluded from statistics. Fixed selection of vertices whose initial height exceeds 1.35 m. Metric: `norm(x[t] - (x[t-1] + x[t+1])/2)` removes constant-velocity displacement but may also measure physical vibration. Full results and initial-state hashes: [jitter-validation.json](jitter-validation.json).

| Scene | P95 before → after | RMS before → after | Peak after |
|---|---:|---:|---:|
| T-shirt, 2,454 vertices | 0.701 → 0.084 mm (**−88%**) | 0.307 → 0.045 mm | 0.860 mm |
| T-shirt + private apron, 3,139 vertices | 0.729 → 0.111 mm (**−85%**) | 0.320 → 0.056 mm | 0.695 mm |

The plane fixture no longer shows the bounce cycle: maximum measured normal displacement after warm-up is below 0.001 µm. Tests use a wider 1 µm tolerance. The first assertion on **3D** displacement failed because of a few micrometres of tangential relaxation; the contact test therefore measures the normal component explicitly, without claiming to freeze sliding. Translating the whole scene by over 100 m changes results by less than 2 µm. Supported cloth retains about 0.22% extension after five seconds. A fresh T-shirt after four seconds has maximum seam gap 2.08 mm and stretch P95 3.88%.

Five-second native measurements after warm-up give **60.01 frames/s and 60.41 physical steps/s** for the T-shirt (1.007×; bounded clock catch-up). For the apron: **59.93 frames/s and 50.18 physical steps/s**, or 0.836×: **the realtime gate fails**. Headless timings vary with workstation load and do not establish controlled throughput gains. The apron remains expensive and some spikes remain. No universal absence of jitter, full convergence or friction is claimed.

## Checks and reproduction

78 CPU tests and three opt-in GPU tests pass; formatting, CPU/desktop Clippy and five templates pass. Native mouse grabbing passes. GPU forces/energies on a draped T-shirt are compared with CPU AD over two steps: maximum absolute gradient error 1.07×10⁻⁴ N. Native captures were inspected. CPU contact tests check activation within the same step, no force before `r`, original bounds and cached/fresh detection equivalence.

```powershell
.\scripts\check-cpu.ps1
cargo clippy --locked -j 2 --all-targets -- -D warnings
cargo build --release --locked -j 2 --bin cloth-fit --example motion_probe

# Explicit GPU use, outside the ordinary suite:
cargo test --release --locked -j 2 --test gpu_motion -- --ignored --test-threads=1 --nocapture
.\target\release\examples\motion_probe.exe --project path/to/fitting-project.json --output output/motion --gpu --steps 180 --iterations 16
python scripts/analyze-motion.py output/motion --min-y 1.35 --warmup 60
.\target\release\cloth-fit.exe --project output/motion/fitting-project.json --steps 0 --ui-perf-smoke --output output/motion-ui
```

`motion_probe` defaults to CPU and buffers accepted states/metrics without modifying the source project. These large traces are diagnostics, not a new editing format. Writes are outside `step_ms`; the tool does not measure window cadence. The Python script uses only the standard library. To compare versions, retain the same input, check state hash/settings and analyze the same vertices. The pre-correction reference is `f385ed0`, instrumented with the same example; private snapshots/traces are not distributed in Git.
