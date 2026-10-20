# Motion diagnostics and a rejected jitter correction

**Status: physics correction reverted on 16 October 2026 after user testing.** The owner observed jerky stress release, impaired sliding and pieces that seemed stuck after dragging. Lower settled-motion RMS did not imply better cloth behavior. Jitter remains unresolved.

## Current behavior

The CPU and GPU use the adaptive inertial starting guess and local line search that preceded this experiment. Original OGC movement balls, collision processing, sparse f64 refinement, timestep, damping and iteration settings are unchanged. Earlier contact-search and draped-scene performance optimizations remain.

The rollback removes the complete-objective prediction rejection, its component graph, three GPU dispatches, component metadata and `prediction_rejected` metric. It also removes the accompanying shortcut for exactly unchanged f32 line-search candidates, restoring the previous search path. There is no replacement filter, extra damping or sleeping mechanism.

## Passive motion detector

`src/motion.rs` observes every accepted physical step on either backend. It measures `|(v_new-v_old)*dt/2|`, equivalent to the position second difference at fixed dt, using a 0.5-second exponential history. Constant-velocity drift is removed. Reported amplitudes use metres, speed uses m/s.

An oscillation flag requires RMS amplitude at least 20 µm, repeated velocity reversals, little net drift and 0.25 s of persistence after 0.5 s of history. These are diagnostic heuristics, not material parameters or proof of numerical instability. Real fabric vibration can trigger the indicator. It neither sleeps nor changes the solver. Rebuild/reset the monitor on external state edits or remeshing; solver generations already do so. A timestep or time discontinuity resets history.

**Properties → Simulation → Motion / jitter** shows RMS, maximum vertex RMS and the number of persistently oscillating vertices. **Inspect jitter hotspots** pauses and marks faces around up to twelve strongest detected vertices. **Export report** includes the last motion metrics. Hotspots use panel IDs and material UVs, not persistent mesh indices. A paused file alone does not contain a history of oscillation: run it briefly before inspecting.

## Why the experiment was rejected

The experiment compared the complete implicit objective of the predicted and previous configurations. If prediction raised that objective, local minimization restarted from the previous positions, independently for connected energy components. Material, sewing and moving contacts connected those components, which could encompass an entire garment.

The objective and gravity were unchanged, but a finite-budget solver does depend on its starting configuration. Repeated rejection can discard useful collective motion that local iterations do not recover smoothly. This is consistent with the user's observed sticking and intermittent release; the exact interactive trajectory was not captured, so it is not a complete causal reproduction.

The private settled scene had 6,731 vertices and 12,682 triangles at 118.3667 simulated seconds, with dt 1/60 s and 16 iterations. The rejected implementation reduced the six-second position-second-difference RMS from 0.03257 mm to 0.001777 mm (94.54%). Contact comparisons and free-fall tests passed. Those checks were insufficient: they did not establish natural release and sliding during editing.

The [historical measurements](settled-jitter-validation.json) are retained as evidence of a **rejected experiment**, not current quality or performance claims. Executables, source snapshots and private geometry remain under ignored `output/settled-jitter/`. Do not revive this guard based on the RMS number alone.

## Rollback validation

The rollback is checked against the preserved pre-experiment executable and accepted-state trace using the same private input and settings. Its results are recorded separately in [rollback evidence](settled-jitter-revert-validation.json). Ordinary tests remain CPU-only; shader validation does not require a GPU. Hardware comparisons and interaction checks require explicit opt-in.

The restored 360-step trace is byte-identical to the pre-experiment trace: every position, velocity and simulated timestamp matches. Contact counts, minimum distances, seam gaps, stretch and precision-vertex counts also match at every step. The passive detector therefore leaves this trajectory unchanged. All 142 default CPU tests, seven opt-in GPU regressions, three shader checks and desktop all-target Clippy pass. This verifies the rollback on the saved scene, not a new solution to jitter.

Future jitter work must include dragging and release, tangential sliding, stress relaxation, supported/free cloth and interactions across sewn pieces. Report transient response and accepted trajectories alongside contact quality, oscillation and full-step cost. A short displacement after a pointer test does not establish smooth motion.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo build --release --locked -j 2 --bin cloth-fit --example motion_probe
.\target\release\examples\motion_probe.exe --project path/to/settled.json --gpu --steps 360 --output output/motion-check
python scripts/analyze-motion.py output/motion-check --min-y -100 --warmup 60
```

`motion_probe` inherits saved simulation settings unless explicitly overridden and requests the same supported subgroup/timestamp features as the application. It persists the effective settings with its output snapshot. Trace writes are excluded from `step_ms`; this is not a window benchmark.

## Research consulted during the rejected experiment

Anka He's [VBD implementation note](https://ankachan.github.io/posts/2026/03/implementing-vbd-properly/) discusses local objective descent and why diagonal stiffness damping against absolute velocity can spoil free fall. This experiment did not implement its internal-variable damping model or add Rayleigh damping. The [OGC project](https://graphics.cs.utah.edu/research/projects/ogc/) remains the contact reference; its performance claims are not ClothCraft measurements. Existing [contact assumptions and seam exclusions](ogc.md) still apply.
