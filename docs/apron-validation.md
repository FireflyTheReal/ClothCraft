# Added-panel motion and sewing correction — 2026-10-14

User reproduction: a newly drawn rectangle stayed almost still while an existing shirt moved. The rectangle was compiled into the physics mesh and was unpinned. Two separate failures made the workflow confusing: GPU VBD omitted the CPU solver's collective inertial prediction, and the two highlighted edges were only a pending seam preview.

## Changes

- GPU `predict` initializes moving vertices from the inertial target, including gravity and damping, clipped to the original OGC balls with their f32 safety margin. Local energy-checked VBD follows. Fixed vertices remain fixed; contacts and centers are not discarded or recentered.
- Physical edits rebuild solver/contact caches but transfer compatible panels' positions and velocities by panel ID and local mesh. Adding a rectangle or a seam preserves the shirt's current drape. New/remeshed/rearranged panels start at their own arrangement; cancelling a placement restores its previous state.
- **Coudre et simuler / Space** commits a pending pair before advancing. Unequal edge lengths require explicit easing; failure remains visible and leaves simulation paused. **Valider la couture** also works without starting physics.
- Candidate construction uses conservative AABB distance bounds instead of duplicating exact closest-feature evaluations. Exact distances, feasible regions and OGC bounds still update every step. No cap silently drops candidates.

## Regression evidence

Windows, RTX 3090 / Vulkan, two CPU jobs/threads. Ordinary checks are CPU-only. Compact reports: [apron-validation.json](apron-validation.json).

- **71 CPU tests pass**, Rustfmt, CPU/desktop Clippy with warnings denied, and five template fixtures validate.
- The opt-in hardware regression failed before the prediction fix: a 6×4 free panel moved only 0.920 mm vertically instead of the analytic 2.659 mm on its first step. After correction, 6×4 and 20×12 grids pass the translation/shape check with reused GPU allocations. A subsequent large-velocity step stays within its original trust regions. This checks motion and buffer growth, not full garment convergence.
- **31 native pointer actions pass**, including rectangle/Bezier editing, mixed 2D/3D edge selection, pending seam commitment on Play, undo/redo, gizmos, export/reload and actual new-panel motion over at least 0.2 simulated seconds.
- In the private three-panel reproduction (3,139 vertices / 5,823 triangles), mean rectangle drop over ten identical steps changes from **0.318 mm to 37.715 mm**. The original project is kept unchanged locally; it is not a public benchmark fixture.
- Its pending edges measured 660 mm and 560 mm. A separate local copy records the intended reversed seam with easing enabled and preserves all saved positions/velocities exactly. Thirty accepted steps reduce the new seam's maximum gap from approximately 361 mm after step one to 180 mm. **This is an assembling seam, not a closed or fitted apron.**

## Performance and remaining limits

The previous Turbo/board realtime numbers preceded this physical correction and are historical. Correct motion causes more broad-phase rebuilds. At unchanged 25 mm mesh, eight iterations and 1/60 s steps, the old T-shirt snapshot gives:

| Measurement | Corrected GPU, before AABB candidate change | Final AABB candidate change |
|---|---:|---:|
| 120-step headless mean, full wall time | 27.131 ms | 25.233 ms |
| Actual window cadence | 60.009 frames/s | 59.999 frames/s |
| Delivered physical cadence | 51.835 steps/s | 52.999 steps/s |
| Simulated/wall time | 0.864× | 0.883× |
| UI frame interval P95 | 18.332 ms | 19.453 ms |
| Realtime acceptance (≥55 frames/s and ≥0.95× physics) | **Fail** | **Fail** |

These are short local runs, not a statistical speedup claim. Each starts from the same saved 6 s snapshot, with one second of window warmup and five measured seconds; no concurrent compilation. The two headless trajectories end with identical reported seam gaps (maximum 13.944 mm). Enlarging candidate cache skin was tried and discarded because it increased cost. The three-panel apron assembly costs 40.099 ms per step over 30 steps, so it is heavier still. Detection dominates; the last apron step spends 36.408 ms in detection/coloring versus 1.288 ms of hardware GPU compute.

The fix establishes that newly created cloth participates in physics. It does **not** certify garment fit, long-time stability, seam closure at eight iterations or realtime on all scenes. Friction is absent; unsupported garments can slide down the avatar. A budgeted local solver is not a converged implicit solve. Future optimization must preserve the free-fall regression rather than restoring the old artificially slow motion.

## Reproduce

```powershell
.\scripts\check-cpu.ps1
cargo clippy --locked -j 2 --all-targets -- -D warnings

# Explicit GPU use, excluded from ordinary checks:
cargo test --locked -j 2 --test gpu_motion -- --ignored --test-threads=1
cargo run --release --locked -j 2 --bin cloth-fit -- --steps 0 --ui-authoring-smoke --output output/apron-authoring

# Local historical snapshot used for the comparison, when present:
.\target\release\cloth-fit.exe --project output/turbo-repro/tshirt/fitting-project.json --gpu-sim --steps 120 --output output/apron-perf-profile
.\target\release\cloth-fit.exe --project output/turbo-repro/tshirt/fitting-project.json --steps 0 --ui-perf-smoke --output output/apron-perf
```

For a new machine without the saved snapshot, `scripts/turbo-perf.ps1` generates scenes and reports current physics/window cadence; those new trajectories are a different benchmark. Treat a failing performance exit code as a failed performance target, not a reason to skip correctness checks.
