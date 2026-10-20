# Coupled seam refinement experiments

18 October 2026. **Research opt-in; not enabled in the editor or normal CLI. Jitter is not fixed.** The saved Garment is a reproduction case, not an input to a special-case rule. No seam name, panel name, vertex number or file path selects solver behavior.

## Method and boundaries

`src/vbd/seam_blocks.rs` builds components from the nonzero coefficients of the compiled sewing quadrature. It includes all free vertices connected through those coefficients, including interpolation between nonmatching meshes. Pinned vertices remain fixed. This cache is constructed only when `VbdSolver::set_seam_block_sweeps` enables the experiment; zero is the default.

After the normal VBD iterations, each component receives an additional search step:

1. Assemble the exact current objective gradient, with the same implicit inertial target, materials, integrated sewing, bending, contacts and grab.
2. Solve an approximate coupled system using block-preconditioned conjugate gradients: exact full sewing Hessian, coupled membrane Gauss–Newton curvature, and local bending/contact curvature. The membrane diagonal subtracted from the local Hessian is analytic and checked against automatic differentiation. Nonpositive axial search curvature is omitted; local indefinite search blocks receive a Gershgorin shift. These are search approximations; the actual energy and gradient are unchanged.
3. Limit the common step length to the **original** OGC balls, then backtrack against the complete affected nonlinear objective. Each shared triangle, hinge, stitch, contact and grab is counted once. An accepted refinement must strictly decrease that objective. There is no position projection after acceptance.
4. Order components using material, sewing, contact and grab dependencies. Only independent components run concurrently. PCG has a 64-iteration cap and a relative residual target of `1e-6`; reaching the cap is not reported as convergence. The outer descent and energy tests remain necessary.

The experimental work runs in f64 on CPU, also when invoked after the hybrid GPU solve. No GPU kernel, constitutive parameter, damping law, timestep, prediction, contact candidate or collision exclusion changes. A rejected refinement leaves the ordinary VBD result intact. This is not the earlier rejected prediction guard, and it adds no sleeping, smoothing or velocity clipping.

The component contains sewing participants, **not the surrounding full cloth patch**. Couplings to external cloth vertices are fixed during the refinement, while bending/contact cross-Hessian blocks are omitted from the search matrix. This is a significant limitation. Search regularization also means that finite-iteration directions need not be rotation-equivariant, even though the physical energy and its derivatives retain their invariants.

`VbdMetrics::seam_blocks` reports attempts, acceptances, bound-limited directions, objective reduction and CPU time. The existing CPU `residual_rms_n` is sampled during the normal vertex sweeps, before the extra refinement; it is not a final coupled residual. A decrease in the within-step objective does not prove mechanical energy decreases across timesteps.

## Experiment sequence

Three prototypes were evaluated on the same saved Garment and unchanged simulation settings:

- Disjoint two-vertex Newton blocks: about **11.6 ms extra CPU work per step**, no useful reduction in settled oscillation. Rejected; only the ignored research snapshot remains.
- Full sewing components with local membrane curvature: about **4.0 ms extra**, still no reduction in settled oscillation. Rejected for activation.
- Full sewing components with coupled membrane Gauss–Newton curvature: the implementation retained here for further experiments. About **5.8–5.9 ms extra** on this machine. The rest-only run still fails the jitter/performance gate.

All these measurements are interleaved full physics steps with two CPU workers. Offline contact/energy audits run between steps and are excluded from `step_ms`. Background load varies between runs; compare branches within the same run. These are not native-window throughput measurements, and they do not establish 60 steps/s.

The matched-control harness also compares 16 normal iterations with 64. On the Garment pull/release test, the final motion-monitor value drops from about **30.5 to 13.4 µm** with 64 iterations, while mean step time rises from **18.4 to 35.5 ms** in that run. This supports investigating convergence but does not identify the sole cause or supply a realtime solution. The 64-iteration branch is a comparison, not a certified converged answer. Motion highpass includes real physical acceleration.

Detailed phase and per-seam measurements are recorded in [validation evidence](seam-block-validation.json). Raw captures, geometry, older prototype sources and binaries stay ignored under `output/seam-block-research/`.

For the retained membrane-coupled candidate, the highpass RMS over the rest-only run after 60 warmup steps is **32.63 → 34.24 µm** globally and **167.89 → 192.69 µm** on `seam_5`. After release it is **30.65 → 26.51 µm** globally, but **170.36 → 169.32 µm** on that seam. The improvement in the global release metric does not repair the settled failure.

Small CPU fixtures also show why less highpass motion alone is insufficient. RMS position disagreement with the 64-iteration branch during release is:

| Fixture | Normal 16 iterations | Plus coupled refinement |
|---|---:|---:|
| Equal sewn lengths | 5.18 mm | 3.25 mm |
| 15% unequal lengths, matching subdivisions | 4.96 mm | 4.00 mm |
| 15% unequal lengths, nonmatching subdivisions | 5.23 mm | 6.97 mm |

The last case has less highpass motion but **more** disagreement with the higher-iteration trajectory. This rejects activation as a general fluidity improvement. The seamless free-panel pull/release traces remain byte-identical with refinement on/off; the normal Garment rest and pull traces also match between experimental builds. The saved document and installed desktop executable hashes remain unchanged.

## Reproduction and controls

The new `seam_block_probe` runs baseline and experimental branches from identical states, rotates execution order and applies the **same world-space grab target** to all branches. The anchor comes from the baseline at step 120. Default schedule at 1/60 s: 120 settling steps, 60 smooth pulling steps over 40 mm, 30 holding steps, then 150 released steps. `--rest-only` disables the grab. It never writes the input garment.

CPU fixtures include equal/unequal sewn lengths, mismatched edge subdivisions and a seamless free panel. The example is CPU by default, even in a desktop build. GPU use requires `--gpu`.

```powershell
$env:CARGO_INCREMENTAL = '0'
$env:RAYON_NUM_THREADS = '2'
cargo run --locked --release --no-default-features -j 2 --example seam_block_probe -- --output output/seam-blocks/unequal --reference-iterations 64
# Repeat with --nonmatching, --ease-ratio 1, or --free and separate output folders.

# Explicit hardware experiment only when GPU use is authorized:
cargo run --locked --release --features desktop -j 2 --example seam_block_probe -- --gpu --project path/to/garment.clothcraft.json --panel front --point 0.5 0.08 --output output/seam-blocks/garment
python scripts/summarize-seam-blocks.py output/seam-blocks/garment --output output/seam-blocks/garment/analysis.json
```

`annotations.json` is a transient map for the accompanying exact mesh/trace, not a garment document format. Do not reuse its vertex indices after remeshing. The analysis script reports phase motion, seam motion, energy changes and attachment travel; it can compare a reference trace only when the initial state and control sequence agree. Energy changes under moving grabs include external work and must not be interpreted as numerical energy creation.

Every experiment independently rebuilds contacts for accepted endpoint diagnostics and checks the final state for intersections using the existing local sewing exclusions. The solver also checks its movement balls on every step. These checks are evidence on the tested trajectories, not a proof for arbitrary scenes.

## Validation

154 tests passed in the full CPU suite. After test-only cleanup, all seven seam-block tests passed, including one added anisotropic PCG regression: 155 distinct passing CPU tests in total. CPU all-target Clippy with warnings denied, formatting, and the six template validations pass. The seven new tests cover stiff common motion and balls, nonmatching coupled systems, anisotropic search rows, exact membrane diagonals/rotation, objective finite differences with active contacts, topology/pins, and exact no-op behavior when disabled or seamless. No test invokes a GPU by default.

## Decision

Keep refinement **off by default**. Neither extra damping nor a file-specific correction has been installed. Do not enable it on the strength of a smaller final motion number after one drag: settled behavior, release response, collisions and step cost must all improve together.

The next discriminating experiment is a small complete material patch spanning a sewing junction, compared against the same patch with fixed surroundings and against a higher-iteration whole-cloth solve. Measure final force residual and residual work, including bound reactions, before deciding whether to enlarge the coupled region or address contact refresh. A hardware implementation is justified only after that numerical experiment passes; porting the rejected candidate alone would make an ineffective correction faster.
