# Simulation continuity and editing feedback

16 October 2026. Follow-up to [the small-contact precision investigation](precision-stall.md).

## Reproduced pause

The updated private garment contains 6,731 vertices, 12,682 triangles and five panels. Its stored state starts at time zero. The previous executable stops after 241 accepted steps (4.017 simulated seconds): the next contact detection falls below the old, global 2 micrometre GPU cutoff. This is a deliberate rejected step, not an idle worker or a hidden pin.

That global f32 cutoff is replaced by precision partitioning already used for tiny displacement balls. All moving vertices of a tiny VF/EE pair inherit its small bound and are retained in f64. They are omitted from GPU solve dispatches, so a singular rounded f32 version of that pair cannot run a local update. Exact pinned positions are also retained. Two sparse f64 sweeps use the same implicit objective and original collision balls after readback. GPU readback now has a strict original-ball check, without the former 10 nm tolerance. A numerically unresolved separation at or below 0.1 nm still rejects the step; no contact is ignored to keep the clock running.

The workspace shows a persistent **Simulation paused** explanation when the worker rejects a step, with Inspect and Copy details actions. The status bar measures delivered simulated time over wall time and displays physical steps/s and simulation speed. It does not derive physics throughput from window FPS or compute timestamps.

## Cost reductions

- The temporal cache stores pairs of BVH leaves. Rebuilding it no longer expands every nearby leaf into individual VF/EE pairs. Every step first checks current leaf boxes, then current primitive boxes, then evaluates exact f64 geometry. Shared-vertex and local seam exclusions are applied before geometry. Fixed/fixed rejection, both VF directions, canonical EE operand order and conservative cache coverage are preserved. Candidate vectors remain unbounded: there is no overflow truncation.
- Static GPU energy terms remain resident. Only contact/grab terms are uploaded each step; topology/material changes and buffer growth upload the static prefix again. Incidence ranges bound the live tail after shrinking.
- Devices advertising a fixed 32-lane subgroup can use shuffle reductions with the same addition partners/order as the portable workgroup reduction. Other devices retain the synchronized portable shader. Shared-memory publication and uniform control still synchronize; there is no assumed cross-workgroup barrier.
- The original eight-primitive BVH leaves and conservative two-radius motion skin are retained. Smaller leaves, topology bit masks, balanced traversal frontiers and padded f32 broad-phase experiments were measured and rejected when their extra traversal or scan cost outweighed savings. They are not additional available backends.

The coverage argument is unchanged: reuse requires every moving vertex to remain within `s` of its cached center and the query radius to stay within cached `R`. Each primitive remains within `s` of its cached geometry, so a pair closer than the current query radius was within `R + 2s` at construction. Its enclosing leaf boxes were at least as close. A BVH refit does not change leaf membership. Leaf/primitive distance tests are lower bounds only; exact distances and movement balls are recomputed every step. `candidate_blocks` counts cached leaf pairs; `candidate_pairs` counts the expanded possibilities before topology/current-distance pruning and is not directly comparable to the old filtered primitive count.

Performance is measured with unchanged resolution, dt = 1/60 s, 16 iterations, materials, seams and contact parameters. GPU timestamps, complete step cost and actual window cadence are separate quantities. Timing evidence is local under `output/realtime-investigation/`; the private garment is not committed.

## Measurements and remaining limit

RTX 3090, Ryzen 7 2700X, two Rayon threads. Other desktop applications were active; runs were sequential, without compilation alongside them. [Recorded results](realtime-validation.json) retain both repeated comparisons rather than selecting the fastest run. The baseline is the saved executable at the start of this investigation, including the preceding sparse f64 patch, not an unmodified historical Git revision.

| Updated private garment, first 240 steps | Before, two runs | Retained change, two runs |
|---|---:|---:|
| Mean complete step | 34.92 / 43.08 ms | 28.02 / 28.32 ms |
| Complete-step P95 | 58.51 / 77.06 ms | 35.71 / 36.51 ms |
| Mean candidate-cache reconstruction | 25.20 / 30.98 ms | 5.01 / 5.15 ms |
| Reconstruction P95 | 32.41 / 41.18 ms | 6.58 / 6.25 ms |

All four final position/velocity/time states match exactly after 240 steps. Both versions rebuild 123 times; the gain comes from cheaper reconstruction, not stale collision distances or fewer detections. Bit-mask variants were also measured and rejected. The eight-primitive leaf cache spends more work scanning than the old filtered primitive list; the improvement is in complete step cost and reconstruction tails, not every individual phase.

The updated scene then completes **1,800 steps / 30 simulated seconds**. Mean complete step: 34.63 ms, P95: 44.30 ms. Final maximum seam gap: 4.16 mm; rest-area extension P95: 5.53%, maximum: 29.69%. A fresh CPU startup intersection/separation check and contact probe accept the final saved state, with minimum included separation 0.144 micrometres. This is evidence for this run under the documented local seam exclusions, not an unconditional intersection or fit guarantee. The large maximum extension still warrants inspection.

| Actual native window, 5 s after warmup | Physics steps/s | UI frames/s | Simulation speed |
|---|---:|---:|---:|
| Reference T-shirt | 60.41 | 60.01 | 1.007× |
| Reference dress | 58.89 | 60.09 | 0.981× |
| Updated private garment | 32.07 | 59.95 | 0.534× |

**60 physics steps/s is not achieved on the private garment.** The first two scenes pass the existing 0.95× / 55 FPS window criterion; that tolerance is not a strict 60-step guarantee. The dense montage fails the real-time criterion. Its remaining dominant cost is the CPU contact scan; GPU timestamps alone are roughly 5 ms during the 240-step comparisons. Further work needs a measured reduction of that CPU phase or a conservatively validated parallel detection backend. Reducing iterations/resolution or widening seam exclusions was not used to reach these numbers.

## Sewing at the cursor

Pick A, then B in either canvas. A nearby card shows both rest lengths, their difference, direction and the explicit **Allow different lengths** choice. F or Shift+wheel reverses B; E toggles easing. A third click anywhere in either canvas commits a single-edge pair. The click is consumed and does not also pick another edge. Invalid sewing keeps the preview and shows its validation error. Enter remains available.

The card stays inside the selected canvas so it cannot cover the inspector's range/chain controls. The existing 55-action editing regression exposed this overlap; the corrected full scenario passes, including M:N seam editing and actual new-panel motion.

For ordered chains, Ctrl+click in either canvas or Ctrl+Enter commits; ordinary edge clicks continue adding segments. Existing range handles remain draggable. Esc cancels a draft.

Click a sewn boundary to select the seam at that material position. The nearby card provides Edit/Delete, and Del removes the selected relationship. If several partial seams share a boundary, the card lets you choose their IDs. Ctrl+Z restores a deleted seam. These controls use existing transactional authoring commands, so removal preserves the compatible drape.

## Selection

**Box / Lasso** and **Panels / Points** are visible above the 2D board in Edit. Drag empty board space; in Points mode, dragging inside the active piece also starts a selection. Shift adds. Lasso containment uses the even/odd rule and panel selection intersects actual cloth triangles, so selecting empty cutout space alone does not select a piece.

Selected contour anchors have blue fills, white outlines, rings and point numbers. The preview updates while drawing the selection. Selected anchors are also marked at their current visible positions in 3D. These are editable contour anchors, not persistent simulation vertex indices. Group movement remains a 2D layout operation. Box/lasso gestures in the 3D viewport, through-selection and mesh-face editing are not implemented by this pass.

The native `workflow` smoke exercises lasso/box point selection, mixed 2D/3D sewing, unequal-length rejection, cursor-local direction/easing, click commitment, seam deletion and undo. Run it explicitly with `CLOTHCRAFT_AUTHORING_SCENARIO=workflow` and `--ui-authoring-smoke`; it opens a GPU window. Default numerical checks remain CPU-only.

## Reproduce

```powershell
# CPU-only numerical and tooling checks
.\scripts\check-cpu.ps1

# Explicit GPU checks: free fall, topology changes, supported cloth, tiny gaps,
# translated geometry, and portable/subgroup path agreement.
cargo test --release --locked -j 2 --test gpu_motion --test gpu_precision -- --ignored --test-threads=1

# Explicit GPU timings on your own saved project, preserving its settings.
cargo run --release --locked -j 2 --bin cloth-fit -- --project path/to/garment.json --gpu-sim --steps 240 --threads 2 --output output/contact-timing
cargo run --release --locked -j 2 --bin cloth-fit -- --project path/to/garment.json --steps 0 --threads 2 --ui-perf-smoke --output output/window-timing

# Real pointer input in a native window; no private garment needed.
$env:CLOTHCRAFT_AUTHORING_SCENARIO='workflow'
cargo run --release --locked -j 2 --bin cloth-fit -- --steps 0 --ui-authoring-smoke --output output/sewing-workflow
Remove-Item Env:CLOTHCRAFT_AUTHORING_SCENARIO
```

Five opt-in GPU tests pass, including 0.2 micrometre initial contact and a scene translated by 100 metres. Portable and subgroup gradients are checked against f64; topology growth/shrink exercises resident-buffer invalidation. The native scenarios pass 21 workflow, 23 quick-tool and 55 full-authoring pointer actions. Ordinary numerical tests never request a GPU.

CPU validation: `scripts/check-cpu.ps1` passes 137 tests, CPU Clippy and six template fixtures. Desktop CPU checks pass 19 library tests, 42 UI tests and both WGSL variants; the optional layout-image exporter remains ignored.

All-target Clippy (`-D warnings`), formatting and the final release build pass. The GPU growth test was extended with a smaller third topology and rerun successfully to check stale-tail safety after shrinkage.
