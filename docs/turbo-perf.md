# Turbo performance — 14 October 2026

**Historical measurements before GPU motion corrections.** [Apron validation](apron-validation.md) exposed missing inertial prediction; [the subsequent artificial-stretch pass](adaptive-initialization.md) replaced systematic free-fall prediction with adaptive initialization. The current default is 16 iterations. Results below compare historical versions and do not establish current performance or quality.

**Realtime was measured in the workspace on both assembled garments**, RTX 3090 / Vulkan, two CPU threads. Mesh, eight iterations, dt 1/60 s and material/contact laws were not reduced to obtain the gain. Detection remained CPU work; this was not a fully GPU OGC port.

## Matched-settings comparison

The old executable was retained before modification. Both executables start from identical `output/ogc-validated/{tshirt,dress}/fitting-project.json` snapshots at six simulated seconds. 120 additional steps, 30 mm mesh target, same parameters. Means include every step, including the first, and metrics writes. P95 uses upper rank; no concurrent compilation during this comparison.

| Measurement | T-shirt before → after | Dress before → after |
|---|---:|---:|
| Vertices / triangles | 1,744 / 3,212 | 2,808 / 5,324 |
| Complete mean step | **50.57 → 6.18 ms** | **56.91 → 5.35 ms** |
| Speedup | **8.18×** | **10.64×** |
| Median | 48.98 → 4.21 ms | 54.66 → 4.31 ms |
| P95 | 59.56 → 14.17 ms | 69.78 → 5.31 ms |
| Maximum after | 17.37 ms | 16.32 ms |
| Detection + coloring, median | 32.76 → 1.76 ms | 37.66 → 1.47 ms |
| GPU compute timestamp, median | 13.21 → 1.11 ms | 13.50 → 1.07 ms |
| Final maximum seam gap | 2.440 → 2.398 mm | 2.265 → 2.261 mm |

Mean computation throughput is about 162 and 187 steps/s, not displayed frames/s. GPU clocks were not locked. These local measurements are not universal guarantees.

## Native window and default mesh

Full reproduction: `scripts/turbo-perf.ps1 -Steps 360 -MeshMm 25 -UiPerf`, assembling separated panels, then opening/measuring the workspace. Eight iterations retained. One second warm-up, five measured seconds, egui's **unique** frame counter and physical time delivered by the worker. This measures window cadence, not hardware display scanout.

| Measurement at 25 mm | T-shirt | Dress |
|---|---:|---:|
| Vertices / triangles | 2,454 / 4,570 | 4,094 / 7,800 |
| Assembly: 6 simulated seconds in | 4.445 s | 4.309 s |
| Complete assembly mean | 12.35 ms/step | 11.97 ms/step |
| Assembly P95 | 26.08 ms | 24.78 ms |
| Window cadence, assembled garment | **60.02 frames/s** | **60.01 frames/s** |
| Measured physical cadence | **59.82 steps/s** | **60.01 steps/s** |
| Simulated / wall time | **0.997×** | **1.000×** |
| Frame interval P95 | 20.43 ms | 18.21 ms |

Reconstruction spikes during assembly still exceed 16.67 ms. The clock catches up short delays, at most four steps; sustained overload slows simulation without changing dt, collisions or iterations. The automated gate requires at least 0.95× physical speed and 55 frames/s; actual measured values are above. This benchmark does not cover large meshes, animated avatars or every mouse interaction.

## Changes and invariants

- BVH with separate cloth/avatar roots, moving refit and primitive-box filtering within leaves. IDs and local seam exclusions remain unchanged.
- Conservative candidate-pair cache with movement envelope and upward-rounded query radius. Distances, forces, feasible regions and OGC radii recompute at every detection. No contact is dropped to meet a fixed budget. Coverage/invalidation: [OGC](ogc.md).
- Two CPU search tasks, deterministic merge of minima and canonical contacts.
- A 32-thread workgroup per vertex assembles forces/Hessians and trial energies. Explicit synchronization, unchanged coloring and the same energy/bound checks. Initial energy is reused rather than recomputed.
- Reused GPU allocations, bind groups, readbacks and timestamps; material terms/static incidence compile once per garment. Retained Arc identity prevents in-place mutation or address reuse from fooling the cache. Dynamic transfers remain.
- Physics worker paced independently of rendering and owning its state. Control changes never reinsert old snapshots. Only one result waits for the interface; generations isolate mesh changes.

## Precision and verification

**57 CPU tests pass**, CPU/desktop Clippy has no warnings, WGSL validates through Naga. New tests: BVH vs exhaustive search after large deformations; cache vs fresh detection after movement/radius changes; a pair entering the radius between rebuilds; rejection of a changed fixed mannequin.

Cooperative GPU forces/energies were compared with f64 AD on contacting snapshots over several steps to exercise the cache. Observed maximum errors: 1.623 mN on the 30 mm T-shirt and 0.487 mN on the 25 mm dress, within the existing 3 mN + 0.3% norm tolerance. Native mouse-event grabbing also passes: attachment accepted, maximum displacement 8.07 mm. Both window captures were produced.

Parallel reduction changes f32 addition order. Equal energies/gradients do not imply bitwise-identical trajectories: after 120 comparative steps, vertex RMS difference is 0.494 mm for the T-shirt and 0.089 mm for the dress; maxima 6.03 mm and 1.19 mm. Eight iterations still do not certify convergence. Friction and industrial calibration remain absent.

## Reproduce and continue

```powershell
.\scripts\check-cpu.ps1
.\scripts\turbo-perf.ps1 -Steps 360 -MeshMm 25 -UiPerf
.\target\release\cloth-fit.exe --project output/turbo-repro/dress/fitting-project.json --gpu-sim --check-gpu-gradients --steps 2 --output output/force-check
.\target\release\cloth-fit.exe --project output/turbo-repro/tshirt/fitting-project.json --steps 0 --ui-interaction-smoke --output output/drag-check
```

GPU tests remain opt-in. [Compact data and executable hashes](turbo-perf.json); detailed logs in `output/turbo/` and `output/turbo-repro/`, ignored by Git. The retained local old executable supports the historical comparison; this pass did not claim a commit/publication.

Remaining priorities at that time: reduce candidate-rebuild spikes, move broad/narrow phase to GPU for large scenes, avoid full per-step transfers and extend validation scenes. Retain the f64 kernel/tests as an oracle. Check the current [handoff](handoff.md) before treating these historical priorities as unimplemented.
