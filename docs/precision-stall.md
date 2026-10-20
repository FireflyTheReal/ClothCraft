# Small-contact stalls in the hybrid solver

Follow-up: [simulation continuity and editing feedback](realtime-follow-up.md) replaces the global 2 micrometre cutoff described in this historical report.

16 October 2026. This fixes a GPU precision stall reproduced from a private diagnostic capture. It does not change seam correspondence, contact exclusions, material parameters, mesh resolution or OGC displacement bounds.

## Cause

Seven unpinned vertices in one skirt panel shared a 1.919005 µm displacement bound. A vertex lay 4.264456 µm from an edge of the same panel. The VF pair and three EE pairs propagated that distance to seven vertices, including vertices without a contact force on that exact feature. Such propagation is intentional: geometric bounds still apply when a feasible-region test rejects a force.

The GPU subtracts a conservative f32 roundoff margin before packing a displacement ball. In this scene that margin was 1.907349 µm. Only about 0.011656 µm remained, below a coordinate's representable spacing. Local iterations repeatedly rounded to essentially the same positions. Changing the translated GPU origin caused tiny apparent motion, but no useful relaxation. Three CPU VBD steps escaped the configuration; 60 old GPU steps did not. This was not a pin or a seam-loop ordering error. Sewing can produce the fold, but the restrictive pair belongs to a single panel.

## Correction

`gpu_vbd.rs` assigns an unpinned vertex to f64 work when its original ball radius is below four times the existing GPU roundoff margin. The GPU treats that vertex as fixed for this pass. Its exact accepted f64 position is retained, rather than round-tripped through f32.

After GPU readback, two sparse sequential sweeps minimize the same implicit objective for these vertices using the CPU local Newton step. Membrane, bending, sewing, contact, inertia and mouse attachments all participate. Each proposal is clipped to the **original** OGC ball and accepted only if its local objective does not increase. The code checks the resulting bound before committing the state. The ordinary CPU path and the precision path share `vbd/local_step.rs`.

There is no unconditional displacement, contact deletion, enlarged collision exclusion, radius floor, or recentering between these sweeps. The GPU's roundoff margin and its existing rejection below 2 µm separation remain unchanged. Exact distance evaluation, coloring and the WGSL kernels are unchanged. State and prediction history commit only after validation.

`precision_vertices` and `precision_ms` expose this work in step metrics. It is included in `solve_ms` and `step_ms`, not in the hardware `gpu_compute_ms` timestamp. Ordinary scenes with no affected vertices do not run f64 sweeps. Dense near-touching configurations can require many CPU vertices and cost more; no silent work cap sacrifices collision checks to meet a frame budget.

## Candidate-cache cost

Once the skirt could move, the original candidate cache rebuilt on 299 of 360 steps. Its motion budget is now twice the contact radius, with the candidate search expanded consistently by twice that budget. If every vertex remains within distance `s` of its cache center, any two primitives can approach by at most `2s`; building candidates out to `R + 2s` therefore still covers every current pair within `rq <= R`. Changing `s` changes work and memory, not physical contact thickness or allowed displacement.

On this continuation, rebuilds fell to 155/360. Mean cached pair count increased from about 339,000 to 635,000; current AABB pruning limits the extra exact work. The final positions, velocities and time were **bit-identical** to the precision correction with the smaller cache. A regression checks two moving primitives approaching from outside the old search envelope, plus equivalence to fresh detection. Every reuse still checks motion and query-radius coverage, and every detection recomputes distances, feasible regions, bounds and forces.

## Reproduction and checks

Use the original diagnostic project as input for both executables. Keep outputs separate; never overwrite the user's capture. The private capture is not committed as a fixture.

```powershell
cloth-fit-before.exe --project CAPTURE/project.clothcraft.json --steps 60 --gpu-sim --threads 2 --output output/stall-before
target/release/cloth-fit.exe --project CAPTURE/project.clothcraft.json --steps 60 --gpu-sim --threads 2 --output output/stall-after

# CPU-only investigation of small bounds and the responsible VF/EE pairs
cargo run --release --locked --no-default-features -j 2 --example stall_probe -- CAPTURE/project.clothcraft.json

# Explicit hardware regression; excluded from the default suite
cargo test --release --locked -j 2 --test gpu_precision -- --ignored --test-threads=2 --nocapture

# Compare motion of marked/restricted vertices and full-step timings
python scripts/analyze-stall.py CAPTURE output/stall-before output/stall-after
```

The synthetic regression starts a patch 4.264456 µm above an avatar plane, checks every accepted step against its original collision balls, rechecks separation and verifies release to more than 20 µm within eight steps. It also runs after a large world translation. Both runs reached 68.833 µm. No private geometry is needed for this test.

The CPU local-step regression checks objective decrease and the original bound under a large load in a tiny ball. The capture's post-run meshes are also passed back through the OGC initial-intersection diagnostic. That diagnostic has floating-point tolerances and documented seam exclusions; it is not an exact all-pairs intersection certificate.

## Captured garment measurements

RTX 3090, driver 616.56, two Rayon threads, Release build, 6,486 vertices / 12,284 triangles, 16 iterations, dt = 1/60 s. Every run starts from the same captured accepted state and uses the same settings. GPU clocks were not locked. No compilation ran concurrently with the timed comparisons.

Three 360-step runs per version (six simulated seconds each), with alternating execution order on the repeated pairs:

| Measure | Before: stalled GPU | Final: precision work + larger cache |
|---|---:|---:|
| Mean full-step time, average of runs | 32.18 ms | 34.25 ms |
| Per-run median range | 33.61–35.21 ms | 28.62–29.58 ms |
| Per-run P95 range | 41.73–42.73 ms | 51.18–52.55 ms |
| Final maximum seam gap | 15.82 mm | 7.55 mm |
| Final positive extension P95 | 21.49% | 10.20% |
| Final maximum positive extension | 106.78% | 64.66% |
| Seven original vertices' net displacement | 0.00017–0.00321 mm | 336.87–382.00 mm |
| Vertices with fresh bounds below 10 µm at the end | 7 | 0 |

The corrected cloth moves and generates a different workload. The final mean is about 6.4% slower than the frozen baseline, while the median improves and reconstruction spikes worsen P95. This is **not** a claim of 60 physical steps/s or a measured window FPS gain. The renderer has its own clock. Precision work averages about 0.18 ms per step; broad-phase reconstruction remains the main cost. The separate precision-only run took 40.19 ms/step, reduced to 34.36 ms/step by the cache change with identical final state. Its candidate construction fell from 14.25 to 9.22 ms/step, while exact/narrow work rose from 8.08 to 8.41 ms/step.

The original seven vertices were already released after one simulated second. A second five-vertex fold appeared during relaxation and also escaped during the longer run. The final scene passes the existing initial-intersection diagnostic and was inspected in the native viewport. No stored seams or rest data were modified. Substantial local stretch and incomplete seam closure remain; this is a precision-stall fix, not a general garment-convergence solution.

Local evidence is under `output/seam-stall-investigation/`: `paired-summary.json`, `long-summary.json`, `comparison-360.json`, the per-step JSONL files, witness probes and the native screenshot. Source captures and replay artifacts remain ignored by Git.

## Final validation

- `scripts/check-cpu.ps1`: 136 CPU tests passed, CPU Clippy passed, all six templates validated. The suite also validates WGSL without invoking a GPU.
- `cargo clippy --locked --all-targets -j 2 -- -D warnings`: passed.
- Opt-in `gpu_motion` and `gpu_precision`: all four tests passed, covering micro-gap release, translation, settled contact, free fall, topology growth, supported cloth and preservation of trust regions. The existing f64 gradient checks also passed.
- Existing assembled T-shirt and dress snapshots, 60 steps each at 16 iterations: old/new final positions, velocities and time are exactly equal, with no precision vertices. Mean full-step times were 19.60 → 19.00 ms and 18.87 → 18.14 ms respectively. P95 rose from 24.33 → 28.35 ms and 23.56 → 27.38 ms: the wider cache has fewer, larger reconstruction spikes. These single-run checks are not universal performance claims.
- The final captured-garment state was reopened in a native GPU window and visually inspected. The original saved document was not overwritten.

All commands used at most two build jobs / Rayon threads. GPU tests remain ignored by default. `CARGO_INCREMENTAL=0` avoided the compiler cache issue recorded in the preceding UI pass.
