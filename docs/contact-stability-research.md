# Contact stability: Box3D, AVBD and recent research

Research review, 16 October 2026. Reading and source inspection only; no GPU use, simulation, dependency installation or production physics change. The owner is rendering. This extends [the energy audit](energy-and-release.md), not the rejected prediction guard.

## What we need to distinguish

Collision feasibility, nonlinear convergence and mechanical energy balance are separate properties. A method can prevent intersections while retaining a force residual, dissipating too much motion or producing oscillation. Stored elastic energy at a balanced, prestressed seam is legitimate. Sustained motion must be explained by external work, released potential energy or numerical/model error.

Our current evidence supports investigating coupled seam modes, but does not isolate the cause of the owner's jitter. The archived Garment loses net energy after its restart transient despite individual increases. Its frozen-contact audit has large cancelling terms. See the existing evidence before interpreting residual work as injected energy.

## Box3D: inspect the solver, not just the resting pile

[Box3D was announced on 30 June 2026](https://box2d.org/posts/2026/06/announcing-box3d/). It is a rigid-body engine. Source inspected at [efca0c184d40fd6a82016983f1601c4d23965867](https://github.com/erincatto/box3d/tree/efca0c184d40fd6a82016983f1601c4d23965867), dated 16 October 2026. Selected files were read; the engine was not built or benchmarked.

| Mechanism verified in source | Relevance to ClothCraft |
|---|---|
| [Substep sequence](https://github.com/erincatto/box3d/blob/efca0c184d40fd6a82016983f1601c4d23965867/src/solver.c#L1247): integrate velocity, warm start, solve with correction bias, integrate position, then relax constraints | Correction-induced velocity deserves separate accounting. A rigid impulse pass cannot simply be appended to our variational cloth solve. |
| [Contact push and relaxation](https://github.com/erincatto/box3d/blob/efca0c184d40fd6a82016983f1601c4d23965867/src/contact_solver.c#L440): overlap correction uses softness and a speed limit; relaxation removes overlap bias/softness while retaining speculative separation bias | The contact repair and physical velocity response are intentionally distinguished. This is not a general energy-monotonicity theorem. |
| [Accumulated normal impulse](https://github.com/erincatto/box3d/blob/efca0c184d40fd6a82016983f1601c4d23965867/src/contact_solver.c#L469) is clamped; an iteration may reduce it | Useful for a future unilateral/dual formulation. Current OGC penalty forces are not accumulated impulses, so this is not a missing clamp to insert. |
| [Feature identity](https://github.com/erincatto/box3d/blob/efca0c184d40fd6a82016983f1601c4d23965867/src/contact.c#L587) controls impulse reuse; new unmatched points start at zero | If we introduce dual state, invalidate it when its physical support changes. Canonical sorting alone does not establish valid warm-start correspondence. |
| [Sleep tests](https://github.com/erincatto/box3d/blob/efca0c184d40fd6a82016983f1601c4d23965867/src/solver.c#L743) consider velocity and positional correction, followed by island sleeping | Resting appearance alone cannot demonstrate a dissipative solver. Do not use cloth sleep to hide the present bug. |

The matching SIMD contact paths were also inspected in [contact_solver_wide.inl](https://github.com/erincatto/box3d/blob/efca0c184d40fd6a82016983f1601c4d23965867/src/contact_solver_wide.inl#L1095). Catto's [Solver2D explanation](https://box2d.org/posts/2024/02/solver2d/) explains Soft Step's substeps, warm starting, compliant constraints and relaxation. It explicitly discusses energy injection from positional-error correction. Its local displacement representation also avoids subtracting large absolute positions to recover tiny velocities.

For performance, [SIMD for Collision](https://box2d.org/posts/2026/07/simd-for-collision/) batches independent SAT edge tests and reports whole-simulation improvements on complex convex hulls, with little benefit for boxes. Our deformable VF/EE workload differs. A transferable experiment is batched CPU geometry with conservative fallbacks, measured against the existing exact f64 path; those published speedups are not ClothCraft predictions.

## Which Roblox work?

The strongest match to “very large-scale collisions” is [AVBD, SIGGRAPH 2025](https://graphics.cs.utah.edu/research/projects/avbd/). The main paper lists Chris Giles at Roblox; the [conference biography](https://s2025.conference-schedule.org/presenter/?uid=470893) confirms his physics role. This identifies a likely reference, not the exact video the owner remembers or proof that Roblox's production engine uses AVBD.

The [Real-Time Live abstract](https://graphics.cs.utah.edu/research/projects/avbd/Augmented_VBD-SIGGRAPH25_RTL.pdf) reports a 110,000-block scene at 3.5 ms solve / 9.8 ms including detection on RTX 4090, with four iterations, and describes demos exceeding a million objects. These are rigid-body research measurements, not thin-cloth self-contact results.

The [main paper](https://graphics.cs.utah.edu/research/projects/avbd/Augmented_VBD-SIGGRAPH25.pdf), especially §§3.4–3.7 and implementation, addresses stiffness disparity using augmented-Lagrangian variables. It also treats energy/momentum introduced while correcting old constraint error and decays reused dual state. Those mechanisms need a consistent formulation for finite elastic seams; hard-constraint stabilization must not silently change their constitutive law.

The official [3D demo](https://github.com/savant117/avbd-demo3d) was inspected at [7701bd427d55ca5d03ea1fdf331912ded9169f4b](https://github.com/savant117/avbd-demo3d/tree/7701bd427d55ca5d03ea1fdf331912ded9169f4b):

- [solver.cpp](https://github.com/savant117/avbd-demo3d/blob/7701bd427d55ca5d03ea1fdf331912ded9169f4b/source/solver.cpp) uses serial body/force loops and a naive quadratic broadphase. This educational demo is not the million-object GPU implementation.
- [manifold.cpp](https://github.com/savant117/avbd-demo3d/blob/7701bd427d55ca5d03ea1fdf331912ded9169f4b/source/manifold.cpp) persists contact state, decays multipliers/penalties and bounds friction forces.
- [spring.cpp](https://github.com/savant117/avbd-demo3d/blob/7701bd427d55ca5d03ea1fdf331912ded9169f4b/source/spring.cpp) has an ordinary finite spring force and an empty dual update. Copying that class would not implement AVBD's treatment of finite-stiffness cloth seams.

Other Roblox references checked were the [2023 intersecting-mesh work](https://about.roblox.com/newsroom/2023/07/roblox-present-3d-motion-rendering-research-siggraph-2023) and [2024 distributed fluid/rigid simulation](https://about.roblox.com/publications/adaptive-distributed-simulation-fluids-rigid-bodies). Their advertised problems differ from prestressed garment relaxation.

## Recent methods worth comparing

| Method and source | What it addresses | Evidence read / limit |
|---|---|---|
| [Planar Divide and Truncate, 2026](https://arxiv.org/html/2604.15513v1) | Direction-aware contact bounds reduce unnecessary restriction of tangential motion | HTML introduction, integration algorithm, results and limitations read. Finite-iteration energy/momentum errors remain, as do OGC potential discontinuities at nonconvex boundaries. The 4M-vertex cloth example reports 8 effective FPS, not 60× realtime. |
| [Geometric Contact Potential, 2025](https://arxiv.org/abs/2402.00719v3) | Contact-potential smoothness, spurious forces and dependence on mesh resolution | Abstract and [official toolkit README](https://github.com/geometryprocessing/GCP-toolkit/blob/main/README.md) read. Toolkit merged into IPC toolkit; not a complete solver or a drop-in OGC patch. |
| [Periodic Anderson Acceleration, HPG 2026](https://diglib.eg.org/items/d6242086-2336-4f27-8b6d-c636b7620fc4) | Globally correlated residuals left by local VBD updates | Publisher abstract retrieved through search; full page/PDF inaccessible in this pass. Reported overhead/results are on their elastic-body experiments, with stiffness ramping; not evidence for our seam/contact pipeline. |
| [JGS2, 2025](https://arxiv.org/abs/2506.06494) | Improve coupled convergence while retaining GPU parallelism | Abstract read; precomputation/subspace/cubature work makes it a larger alternative-backend project. No implementation inspected. |
| [VBD damping implementation, 2026](https://ankachan.github.io/posts/2026/03/implementing-vbd-properly/) | Correct force stencils for deformation-rate damping | Author's technical note, previously read. Full participating velocities matter; absolute-velocity stiffness-diagonal damping can incorrectly resist rigid motion. |
| [Small Steps, 2019](https://mmacklin.com/smallsteps.pdf) | Trade iterations for smaller integration steps | Abstract, introduction and algorithm read. XPBD evidence motivates a comparison; it does not prove that substeps beat our VBD/OGC at equal full-step cost. |

The current [Newton VBD guide](https://newton-physics.github.io/newton/latest/solvers/vbd.html) explicitly describes an experimental solver and a DAT contact path. This is a useful external implementation reference, not a backend available in ClothCraft. The [IPC toolkit 1.6 announcement](https://github.com/ipc-sim/ipc-toolkit/discussions/243) also exposes a Planar-DAT filter; its implementation was not audited here.

## Proposed order for ClothCraft

These are independent implementation proposals, not reported paper results or enabled options.

1. **Locate the numerical contribution.** Extend the existing offline audit with accepted-step seam residuals, contact feature/normal changes, and normal versus tangential displacement lost to truncation. Use the identical saved state and identical absolute grab target path for comparisons. Keep the detector read-only.
2. **Resolve coupled seam motion.** Prototype a small CPU block involving the full weighted seam stencil and every incident energy term. Keep material/rest data, mass, drag and the objective unchanged. Respect each original OGC ball, preserve color dependencies and compare against a well-converged reference. This is the least disruptive continuation of our two-mass evidence.
3. **Compare a compliant augmented-Lagrangian seam formulation.** Treat penalty parameters as numerical variables distinct from material stiffness. Verify that elimination/convergence recovers the same finite-stiffness energy, forces and prestressed equilibrium before using temporal warm starts. Include dual-state invalidation and release tests. No hard-constraint shortcut for elastic seams.
4. **Investigate directional bounds only if truncation is implicated.** Keep this separate from solver convergence work. Replacing balls changes the feasibility argument: establish coverage for both moving primitives, query-envelope motion, finite precision, local seam exclusions and refresh scheduling. Do not just remove the tangential component of the current clamp. Keep the production OGC contract until the alternative has an independent collision oracle.
5. **Evaluate substeps or safeguarded acceleration at equal cost.** Compare 1×16, 2×8 and 4×4 sweeps over the same 1/60 s physical interval. These are equal sweep counts, not equal runtimes. Recompute timestep-dependent terms and account for detection, transfers and waits. Any extrapolated/coarse candidate must remain feasible and pass the existing objective check; restart history on contact/control/topology changes.

If a converged, consistent model still needs physical dissipation, test objective strain-rate damping as described in the energy note. That changes the material's transient response and needs calibration. Do not apply a global velocity multiplier, sleep threshold or energy rejection to make the diagnostic look better.

Acceptance requires rest **and** pull/hold/release, free translation/rotation, sliding, mismatched seams and dense self-contact. Measure physical time advanced, full-step P50/P95, force residuals, work/energy balance, seam gap/stretch and fresh intersection checks. A lower jitter score with delayed release or sticking fails. No speedup or jitter fix is claimed by this research pass.
