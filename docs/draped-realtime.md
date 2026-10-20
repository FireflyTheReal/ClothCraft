# Draped garment performance — 16 October 2026

This pass starts from the private garment at 30 simulated seconds, rather than only measuring its initial arrangement. It retains 6,731 cloth vertices, 12,682 triangles, a 1/60 s time step, 16 VBD iterations and all original material/contact settings. The previous [GPU contact search](gpu-contact-search.md) remains a hybrid: precise f64 contact geometry and movement balls still run on CPU.

## Changes

- `ogc/incidence.rs` stores contact incidence in contiguous offsets/indices, preserving contact order, repeated indices and empty rows. It replaces thousands of per-vertex heap allocations. This is a runtime representation change, not a project-file change.
- `ogc/canonical.rs` sorts compact integer keys for worlds with at most 32,768 vertices. The keys encode the full canonical order, with no truncated identifiers. Larger worlds use the unrestricted comparison sort.
- `ogc/world.rs` specializes direct GPU candidates and cached CPU leaf pairs separately. Both evaluate the same f64 closest features, feasible regions, minima and local seam exclusions.
- GPU candidate records use eight bytes instead of sixteen. One tag bit distinguishes VF/EE; unsupported ID ranges use the complete CPU path. Overflow still discards the partial output and repeats the full search after growth.
- `vbd/coloring.rs` uses a register bit mask for the usual small palette, with the original unrestricted algorithm above 64 colors. Colors and vertex order remain the same as the greedy reference.
- Sparse precision refinement schedules only independent vertices together. Every shared material/contact/grab term preserves the original visitation dependency. This is equivalent to the existing sequential sweeps, not a simultaneous Jacobi update; original movement balls and energy checks remain in force.
- `vbd.wgsl` has a scalar energy path for line-search trials. It retains the same scalar formulas and guards while omitting gradient/Hessian construction. The force probe checks this scalar path against CPU AD energy and the full GPU gradient against CPU AD forces. An unused term field was removed (96 → 80 bytes).
- CPU upload staging and prediction history reuse their allocations. Every active length/range resets after topology growth or shrink. Stretch diagnostics compute deformation lengths directly, without constructing unused normalized fiber frames.

No movement radius, collision pair, iteration or resolution is sacrificed to meet a frame budget. Existing OGC initial-separation assumptions, local sewing exclusions and floating-point limitations still apply. This does not add friction or repair initially intersecting cloth.

## Measuring the whole step

`contact_profile` now separates precise geometry, canonicalization and incidence construction. `gpu_profile` separates coloring, CPU packing, uploads, command encoding, submission/wait, result processing and cleanup. `submit_wait_ms` includes compute; do not add it to the hardware `gpu_compute_ms` timestamp. `post_ms` includes precision refinement and diagnostics. The outer GPU step timer also includes destruction of temporary CPU data.

The CLI metrics stream is buffered and explicitly flushed before stopping its wall timer. Earlier unbuffered JSON writes added many small filesystem calls between physical steps. That change improves benchmark overhead, not native simulation speed. Native `physics_steps_per_second` remains the acceptance measurement; renderer FPS is a separate quantity. The existing smoke gate is only 0.95× simulation and must not be described as a strict 60-step guarantee.

## Evidence

The final build reaches the 60-step target in three native-window samples from the draped garment. Hardware is RTX 3090 / Ryzen 7 2700X, with two CPU workers. Desktop load and GPU clocks are uncontrolled; early repeated old/new short runs showed substantial timing variation.

| Measurement | Before | Final |
|---|---:|---:|
| Full step mean, 1,800 steps from 30 to 60 simulated seconds | 18.87 ms | **14.76 ms** |
| Step P95, same run | 27.43 ms | **17.60 ms** |
| Precise CPU contact processing, including sorting/incidence | 5.42 ms | 3.57 ms |
| GPU search submission/wait/readback | 1.86 ms | 1.59 ms |
| VBD hardware timestamp | 4.42 ms | 4.07 ms |
| CLI total wall time, including its logging | 36.84 s | 26.60 s |

The final full-step timer includes cleanup that the previous build excluded. CLI wall time also benefits from buffered logging; that is not a physics speedup. All 1,800 steps, including initial resource growth, are included in the table. The final positions, velocities and time are **bitwise identical** to the preserved pre-pass executable (IEEE-754 state hashes recorded in the evidence).

| Final native sample | Delivered physics steps/s | Rendered frames/s |
|---|---:|---:|
| Start from the 30 s drape | **60.59** | 59.99 |
| Start from the 60 s drape | **60.59** | 59.99 |
| Repeat from the 60 s drape | **60.85** | 60.05 |

Each window measures five seconds after one second of warm-up. Values slightly above 60 include bounded catch-up by the existing fixed-step worker. **This establishes realtime throughput on these measured states, not a hard deadline for every step or a guarantee on other garments/hardware.** The long-run P95 remains above 16.67 ms.

The exact-contact oracle agrees with the complete CPU detector on 120 steps from each draped state (30 s and 60 s). A fresh CPU probe of the final 60 s state passes initial intersection validation and reports positive minimum separation of 1.08089e-7 m, under the existing local seam exclusions. The force/energy probe passes four contact-scene steps on the reference T-shirt (2,454 vertices); maximum absolute gradient error is 5.539e-4 N.

Validation passes: 140 tests without desktop features, six opt-in GPU regressions, 22 release-library tests, three shader validation tests and six template validations. CPU and desktop all-target Clippy are clean. Library tests overlap between configurations; these counts are not additive. Results are recorded in [machine-readable evidence](draped-realtime-validation.json).

Raw measurements and preserved executables are under ignored `output/draped-perf/`. The private document and its exported geometry are not part of the repository.

## Research considered

- NVIDIA's [synchronization guidance](https://developer.nvidia.com/blog/advanced-api-performance-synchronization/) recommends minimizing unnecessary fences and validating actual overlap. Here, the measured VBD submission/wait time was only about 0.5 ms above its compute timestamp. That evidence favored reducing CPU data handling and GPU work before adding a second compute context. The existing submission-specific waits and required barriers are preserved.
- The [P-Cloth abstract](https://arxiv.org/abs/2008.00409) describes multi-GPU assembly, pipelined integration and a different collision scheme for much larger meshes. Only the abstract was consulted in this pass. It does not establish a drop-in speedup for this single-GPU VBD/OGC implementation; no P-Cloth code or solver was integrated.

## Reproduce

```powershell
$env:RAYON_NUM_THREADS = '2'
# Start from the same draped snapshot for both builds.
.\target\release\cloth-fit.exe --project path/to/draped.json --gpu-sim --steps 240 --threads 2 --output output/draped-compute
.\target\release\cloth-fit.exe --project path/to/draped.json --steps 0 --threads 2 --ui-perf-smoke --output output/draped-window
# Extra CPU detection checks exact contacts and original balls; not a performance run.
.\target\release\cloth-fit.exe --project path/to/draped.json --gpu-sim --check-gpu-contacts --steps 120 --threads 2 --output output/draped-oracle
# Explicit GPU regressions, outside ordinary CPU CI.
cargo test --locked --release -j 2 --lib --test gpu_contact_filter --test gpu_motion --test gpu_precision --test vbd_shader -- --include-ignored --test-threads=1
```

Do not publish a new speed claim from GPU timestamps alone. Compare the same saved state, elapsed physical time, seam gaps, stretch, fresh collision checks and native cadence.
