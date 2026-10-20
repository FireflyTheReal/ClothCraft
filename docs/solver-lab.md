# Cloth solver laboratory

The editor still uses VBD + OGC. This laboratory contains **CPU research adapters**, not a new editor selector or a recommendation to replace the current solver. `src/solver_lab` provides actual stepping implementations with the same compiled garment and `State`. The independent Box3D executable uses a different physical model.

## Implemented experiments

The [consistency investigation](solver-consistency.md) adds `xpbd-primal`
(full-residual Gauss–Newton/PCG) and `avbd-target` (finite AVBD with target
completion, directional contact curvature and coupled polishing). Both are
independent CPU research variants. Original `xpbd` and `avbd-finite` remain as
controls; neither the names nor the new variants imply a production backend.

| Adapter | Material and integration | Contact | Status |
| --- | --- | --- | --- |
| `vbd` | Existing orthotropic membrane, normal-difference bending, weighted seams; production CPU VBD | Existing OGC | Control |
| `xpbd` | The same material energy factored into scalar compliant constraints; sequential XPBD updates | Same OGC energy and conservative movement regions | Experimental safety-limited XPBD hybrid |
| `avbd-finite` | Finite-stiffness ramp from AVBD §3.4, vertex block minimization and §3.5 curvature approximation | Unchanged OGC potential and regions | Partial independent AVBD adaptation; no augmented hard-constraint backend |
| Box3D lattice | Actual Box3D Soft Step; spheres at mesh vertices and distance springs | Native sphere–sphere and sphere–avatar mesh contacts | Different discretization; no cloth face/edge contact coverage |

None implements Coulomb friction for the cloth comparison. Box3D friction and sleep are disabled explicitly. No solver uses private dress IDs to change its physics. Topology and rest coordinates are never reconstructed from the drape.

### XPBD details and limits

The membrane's warp/weft compression and smooth positive-part terms are factored separately, plus shear. Each hinge has three normal-difference factors. Each seam quadrature sample has three weighted positional factors. Their sum `0.5*k*C²` matches the existing material energy, including area, grain, bending weight and seam quadrature. First-order automatic differentiation computes the constraint gradients.

The implementation stores force-valued multipliers: `delta_lambda = -(C + lambda/k) / (sum(dt²/(m*(1+damping*dt))*|grad C|²) + 1/k)`. This is XPBD's usual impulse-valued equation rescaled by `dt²`. Material multipliers reset per timestep. Contact energy is factored as `sqrt(2*U)`; multipliers persist by feature identity within a timestep. Inactive features have no correction. No multipliers are transferred across a backend switch.

All dynamic vertices of a constraint share one feasible step fraction. Both displacement and multiplier increment use that fraction. It is computed against the original OGC region centers, including directional halfspaces. The predictor is also clipped; any clipped sweep triggers contact detection before the next sweep. **Clipping the predictor breaks the unmodified XPBD inertial relation.** This is a collision-safe research hybrid whose finite-iteration material response must be measured; the existence of an XPBD formula alone does not establish implicit-Euler convergence under persistent clipping. This implementation is not promoted to the editor.

### AVBD scope

[Giles, Diaz and Yuksel, 2025](https://graphics.cs.utah.edu/research/projects/avbd/) distinguish augmented hard constraints from finite springs. For finite springs their equation 16 ramps stiffness up to the physical value, with **no dual term**. That is the branch implemented here. Pins remain eliminated degrees of freedom; OGC remains the collision safeguard. We do not claim to reproduce Roblox's internal GPU implementation, hard joints, friction or their rigid-body demonstrations.

Each energy factor starts at 10% of its target stiffness. Between timesteps its previous value is multiplied by 0.99 with that floor. Each sweep adds `10*k_target*abs(C/C_scale)`, capped at `k_target`. `C_scale` is 1 m for linear constraints and 1 for dimensionless strain/normal factors. These are explicit experimental tuning choices, **not the paper's unscaled beta=10**. At a finite iteration budget some terms remain softer than the source material. The audit evaluates the original full-stiffness energy, so cost/quality comparisons expose this difference.

The local Hessian uses the outer product plus a diagonal norm of each scaled constraint curvature column. Contact curvature uses a conservative Frobenius-norm diagonal majorizer. Local steps must decrease their current objective and stay within OGC regions. This is slower sequential f64 research code, without production graph-coloring or GPU kernels.

### Box3D scope

The experiment links the [actual MIT-licensed Box3D](https://github.com/erincatto/box3d) at commit `efca0c184d40fd6a82016983f1601c4d23965867`. Its core is C17; this small harness is C++20. External source lives under ignored `output/research`, not vendored into ClothCraft.

One sphere of radius 1.5 mm represents each vertex, with the compiled vertex mass and locked rotations. Triangle edge springs use the material mean warp/weft stiffness, rest lengths and adjacent triangle area. Barycentric seams become pairwise links between opposite-sign weights. This loses the original coupled sewing operator, orthotropy, nonlinear compression response and bending energy. Native distance joints clamp zero rest lengths to their stability minimum. Native contact is not an OGC substitute: spheres leave holes between vertices through which surfaces may pass. A separate ClothCraft audit checks the original triangles, using the same seam exclusions.

The optional Box3D grab targets one particle through a native spring to a kinematic handle. Its 40 mm trajectory uses 30 quiet, 30 pull, 15 hold steps, then release. It is **not** the barycentric grab used by the Rust adapters. Native trace times are elapsed time from the input checkpoint, not the editor's absolute clock. Compare its timing and failure modes, not equal-model accuracy. Sleep is disabled so an artificially frozen lattice cannot qualify as jitter-free. `CLOTH_LAB_MESH_TRIANGLES` defaults to 4,096 instead of the stock 512; a triangle-buffer capacity warning aborts the run. This cannot compensate for missing cloth surface contact.

## Reproduce

Use an immutable copy of the user's saved project and its adjacent assets. The runner also exports a portable `source.json`, exact initial/final states, protocol and per-step JSONL into a new directory. It refuses an existing output directory. Switching algorithms always cold-starts predictor/contact caches. A saved drape is a checkpoint, not a certified equilibrium.

```powershell
cargo build --release --locked --no-default-features --example solver_compare -j 2
$env:RAYON_NUM_THREADS = '2'
target/release/examples/solver_compare.exe --project path/to/source.json --output output/lab-vbd --backend vbd --steps 180 --pull
target/release/examples/solver_compare.exe --project path/to/source.json --output output/lab-xpbd --backend xpbd --steps 180 --pull
target/release/examples/solver_compare.exe --project path/to/source.json --output output/lab-avbd --backend avbd-finite --steps 180 --pull
```

All adapters use the source timestep, damping and gravity, with 16 sweeps by default. A sweep is algorithm-specific, so equal sweep counts are a budget comparison, not equal work or convergence. Timings encompass accepted steps; fresh geometry audits and JSON serialization are outside the timed region. Report warm-up, accepted physical time, failure, contact separation, seam gap, strain and motion per panel. In the current dress one panel is disconnected: its free fall must not be called garment jitter.

Native experiment, optional CMake/Visual Studio dependency:

```powershell
git clone https://github.com/erincatto/box3d.git output/research/box3d-lab
git -C output/research/box3d-lab checkout efca0c184d40fd6a82016983f1601c4d23965867
$box3dSource = (Resolve-Path output/research/box3d-lab).Path
cmake -S experiments/box3d -B output/research/box3d-build "-DBOX3D_SOURCE=$box3dSource"
cmake --build output/research/box3d-build --config Release --parallel 2
# Add --export-lattice to one Rust run above to produce lattice.txt.
output/research/box3d-build/Release/clothcraft_box3d.exe output/lab-vbd/lattice.txt output/box3d 180 4
target/release/examples/solver_compare.exe --project output/lab-vbd/source.json --output output/box3d-audit --audit-trace output/box3d-states.jsonl
```

Native timestep cost includes state retrieval but excludes trace writing and the independent surface audit. Box3D substeps and Rust sweeps are different units. Four substeps are not equivalent to four cloth iterations. GPU benchmarks require separate explicit authorization; the new adapters are CPU-only. The existing hybrid VBD control is available with `cargo build --release --locked --example solver_compare -j 2`, followed by the same command with `--backend vbd --gpu-control`. Other backend/GPU combinations are rejected. Adapter details are archived in `gpu-adapter.txt`.

## Validation and next gates

### Recorded dress comparison

`solver_compare --record-states` writes `render-geometry.json` and
`positions.jsonl` alongside the existing initial state, protocol and metrics.
Each record is a solver-returned step, with its original double-precision
positions and simulation time. If the independent post-step audit fails, its
last diagnostic state is retained and clearly marked as a stopped run in video;
it is not counted as a validated physical step. Serialization is outside step timing. Rendering
does not run the solver, resample the mesh or smooth the trajectory.

```powershell
cargo build --release --locked --no-default-features -j 2 --example solver_compare
python scripts/solver-video-runs.py --project path/to/garment.clothcraft.json --output output/video-new --steps 350
python scripts/render-solver-video.py --input output/video-new --output output/video-new/comparison-350steps.mp4
```

The recorder preserves an immutable project/avatar copy and runs the five
Rust adapters sequentially with two CPU threads, four sweeps, a 1/60 s timestep,
the same saved state and the same barycentric pull/hold/release. A sweep has
different cost and convergence in each algorithm. These are quality/cost
comparisons of the existing implementations, not a ranking of published methods.
Independent endpoint surface checks run after each accepted step, outside
timings; they supplement the solver's existing movement bounds. Failures remain
in the evidence and are labelled as stopped runs in playback.

The CPU renderer requires Python numpy, numba, Pillow and ffmpeg. It uses a fixed
orthographic camera, a depth buffer, interpolated normals and 1.25x supersampling.
Every column displays the same physical frame. All 350 steps are retained;
30 fps playback of 60 steps/s is half speed, with one-second initial/final holds.
The five columns are VBD, original XPBD/OGC, finite AVBD, full-residual primal
and target-complete AVBD. Box3D's failed particle lattice and the 16-vertex
discrete-contact oracle are not presented as full-dress backends.
The detached panel in the saved dress remains simulated outside the camera.
CPU costs in the cards are full accepted-step means excluding ten warm-up
steps; they are neither editor/GPU timings nor video playback FPS.

The 20 October recorded campaign is under `output/solver-video-20261020`.
The original save hash remains
`e542e066f4bf662149f8603bab47913a60498915689386f0bd51503e4d7aaccb`.
All five adapters reproduce their respective archived 120-step final positions
exactly, including the VBD control from before the extended research pass.

| CPU adapter, four sweeps | Recorded steps | Final maximum seam gap | Final stretch P95 | Endpoint separation audit |
| --- | ---: | ---: | ---: | --- |
| VBD | 350 | 10.7 mm | 14.1% | Pass at every recorded step |
| XPBD / OGC hybrid | 289 | 988.7 mm | 981.2% | Fails on the 289th state; 288 checked states precede it |
| Finite AVBD | 350 | 194.6 mm | 83.2% | Pass at every recorded step |
| Full-residual primal | 350 | 11.8 mm | 17.8% | Pass at every recorded step |
| Target-complete AVBD | 350 | 87.9 mm | 82.7% | Pass at every recorded step |

XPBD's failure is the positive-separation check reporting touching/coplanar
features, not proof of a specifically classified penetration mechanism. Its
last diagnostic pose remains visible with an explicit failure overlay; the
video does not invent the missing 61 simulation steps. The other captures
contain 5.833 seconds of physical time. Mean CPU costs were respectively
252, 587, 1257, 1249 and 1436 ms per returned step. Background workstation
load, compilation and preview generation were uncontrolled; this single
capture campaign is not a precise performance ranking. Four sweeps are not
equal convergence budgets. No production solver or GPU kernel was modified.

The coupled primal result improves substantially between 120 and 350 steps,
but neither the video nor endpoint audits establish equilibrium, absence of
jitter, continuous exact collision freedom, or production readiness. The
video makes delayed motion and deformation differences inspectable. The
separate moving-plane certificate experiment remains a small CPU oracle and
is not applied to any dress column.

See the [measured dress results](solver-lab-results.md). The two new CPU adapters fail the material-quality gate at the tested budget; the Box3D lattice fails surface collision coverage. None is promoted to the editor.

- Factorized material energy versus the existing energy; AD gradients versus finite differences and second-order AD.
- Rest preservation and analytic damped free fall in a regime not limited by collision bounds.
- Floor contact with pull/release; finite state, separation and transactional rejection.
- Saved dress from the same exact state, with quiet and matched pull/release protocols; independent surface checks.
- Before user-facing selection: resolve XPBD's constrained predictor consistency, test AVBD against converged reference forces, add active-contact force/residual histories, benchmark substeps and convergence budgets, and test dense folds, friction and moving avatars.

References actually consulted: [XPBD paper, 2016](https://mmacklin.com/xpbd.pdf), [AVBD project and paper, 2025](https://graphics.cs.utah.edu/research/projects/avbd/), [Box3D source and API](https://github.com/erincatto/box3d). These implementations and scope statements supersede the earlier candidate-only entries in the solver landscape.
