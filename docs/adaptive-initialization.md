# Gravity-induced stretch and seam removal — 2026-10-14

## Diagnosis

The previous collective GPU prediction fixed nearly immobile free panels, but initialized **every** vertex with the full gravity displacement every step. With only eight local iterations, vertices already supported by cloth/contact did not undo that displacement completely. Its remaining error became velocity, producing artificial softness and accumulating stretch. The 200 g/m² reference density was correctly integrated over rest area; mass was not off by a unit conversion.

This is the failure described by **Vertex Block Descent §3.7, option (c), Figure 5**, not an OGC guarantee of realistic material response. The [original paper](https://graphics.cs.utah.edu/research/projects/vbd/vbd-siggraph2024.pdf) was downloaded and its initialization section and equation 17 read for this correction. Prior force/free-fall checks and short FPS checks did not test supported long-term behavior adequately.

## Correction

`src/vbd/prediction.rs` implements a shared CPU/GPU adaptive initial guess. From the previous **accepted** step, estimate acceleration along gravity, clamp its ratio to [0,1], and use that ratio only in the initial guess. For our existing backward-Euler damping, use `(v_new - v_old)/dt + damping*v_new` before projecting onto gravity; this retains full gravity in damped free fall and zero additional acceleration at supported rest.

The implicit inertia target still includes **full gravity** and the mass/constitutive law does not change. No strain cap, position correction after the solve, hidden pinning or weaker gravity is introduced. The proposed guess is clipped to the **original OGC balls**, including f32 safety margins; local energy checks and fixed vertices remain unchanged. With no usable history, the first step keeps the full inertial prediction. History is not serialized and is invalidated by state edits/reloads, solver reconstruction or changes in gravity/damping. It updates only after an accepted step.

This is a better initialization for an incomplete solve, not a convergence certificate. OGC balls may still limit individual vertex motion near contact. Friction remains absent. An unsupported or badly attached panel can still slide or fall; this correction does not invent support for it.

## Observable quality

`garment/diagnostics.rs` measures positive axial extension relative to immutable material coordinates: area-weighted 95th percentile and maximum, in percent. Both CLI metrics and the atelier status bar display these alongside maximum seam gap. These are deformation/assembly indicators, not stress or solver residual. The advanced iteration range now extends to 128 for explicit quality comparisons; **the default is now 16** (previously eight).

Adaptive initialization alone did not make an eight-iteration budget accurate enough on a finely divided supported strip. The new regression first failed: 3.51% mean extension at five seconds, versus about 0.21–0.22% at 16/32/64 iterations. Raising the default to 16 is an explicit quality/cost choice, not a material change. The unchanged-budget T-shirt comparison isolates the initialization fix; final-default measurements must report 16 iterations.

## Removing seams

The **Coutures existantes (N)** list is available at the top of the inspector in every mode, with named endpoints, selection and explicit **Supprimer** buttons. In **Modifier** or **Coudre**, selecting a sewn boundary in either 2D or 3D highlights both linked sides and exposes **Supprimer la couture**. **Suppr** removes the uniquely associated seam when no text field owns keyboard input. If several partial seams share a named edge, use the individual buttons. **Ctrl+Z** restores the relation.

Removal uses the existing transactional command, rebuilds solver/contact caches and preserves compatible positions/velocities. It does not delete the cloth. The native authoring smoke explicitly deletes and restores a seam before completing its other editing, fabric and simulation checks.

## Validation

**76 CPU tests, two opt-in GPU regressions, CPU/desktop Clippy and five template fixtures pass.** The **37-action native authoring smoke passes**, including deleting/restoring a seam while preserving the drape. Screenshots of the inspector and both garments were inspected.

Fresh T-shirt, 2,454 vertices / 4,570 triangles, 25 mm mesh, dt 1/60 s, same 200 g/m² material, gravity, contact and sewing:

| State | Iterations | Max seam gap | Axial extension, area P95 |
|---|---:|---:|---:|
| Before, after 4 s | 8 | 362.51 mm | 51.61% |
| Adaptive initialization only, after 4 s | 8 | 2.18 mm | 8.35% |
| Final default, after 4 s | 16 | 2.61 mm | 4.79% |
| Final default, after 10 s | 16 | 2.19 mm | 4.48% |

The final native-window check delivers **59.99 physical steps/s and 59.99 UI frames/s** on RTX 3090 / Vulkan, with one second warmup and five measured seconds; frame-interval P95 is 21.58 ms. This is the assembled T-shirt check, not a guarantee that every assembly step is under 16.67 ms. Its final-default first four seconds cost 19.04 ms/step; subsequent six seconds cost 17.32 ms/step. Initialization-only timings are exploratory, not a controlled speedup comparison. Some local triangles still stretch more (18.93% maximum at ten seconds), so this is not a calibrated industrial fit.

The private apron reproduction remains harder: after three additional seconds, seam gap 22.34 mm and extension P95 18.68%; after nine additional seconds, 6.84 mm and 8.63%. The window check, started from the three-second result, gives **52.36 physical steps/s, 59.96 UI frames/s**, and **fails** the ≥0.95× realtime criterion. Its unequal seam lengths require gathering and the current penalty/contact model converges slowly. This pass does not certify that assembly, its material response, or all local distortion as solved.

In the supported-strip check at five seconds, mean hem extension is about 0.22% with 16 iterations and 0.21% with 32/64. Eight iterations with the adaptive guess failed the original quality bound (3.51%), motivating the explicit default change. The material, density and gravity were not weakened to pass. All measurements and limitations are in [stretch-validation.json](stretch-validation.json).

```powershell
.\scripts\check-cpu.ps1
cargo clippy --locked -j 2 --all-targets -- -D warnings
cargo build --release --locked -j 2 --bin cloth-fit

# Explicit GPU use only:
cargo test --locked -j 2 --test gpu_motion -- --ignored --test-threads=1 --nocapture
.\target\release\cloth-fit.exe --steps 0 --ui-authoring-smoke --output output/seam-regression
.\target\release\cloth-fit.exe --garment tshirt --mesh-mm 25 --gpu-sim --steps 240 --iterations 16 --output output/stretch-shirt4
.\target\release\cloth-fit.exe --project output/stretch-shirt4/fitting-project.json --gpu-sim --steps 360 --iterations 16 --output output/stretch-shirt10
.\target\release\cloth-fit.exe --project output/stretch-shirt10/fitting-project.json --steps 0 --ui-perf-smoke --output output/stretch-shirt-ui
```

The UI currently uses its own default VBD settings; CLI `--iterations` applies to headless steps. Its default and the CLI default both changed to 16. `scripts/turbo-perf.ps1` now explicitly uses 16 in its headless phase too. The historical eight-iteration tables remain historical and must not be relabelled as this corrected run.
