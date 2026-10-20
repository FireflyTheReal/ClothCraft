# Cloth solver research and comparison plan

18 October 2026. Research and planning only. No simulation, GPU workload, solver activation or executable replacement in this pass. External implementations were not built or benchmarked. Availability below means an official project/repository was inspected, not that its implementation was validated here.

## Recommendation

Keep VBD + OGC as the baseline. Build a repeatable comparison before replacing it or adding another production stabilization. The first independent solver experiment should be continuum XPBD with substeps. A compliant augmented-Lagrangian variant of VBD is the next contender for difficult sewing. Evaluate directional contact bounds separately. Use Newton Physics and ZOZO's solver as external comparators before attempting substantial ports.

These are hypotheses to test, not available ClothCraft backends or a claim that one family is universally faster. The existing [solver contract](solver-challenge.md) remains authoritative. The source review covers primary author pages, arXiv papers, SIGGRAPH/Eurographics/HPG publications, Disney Research, Erin Catto and official repositories. It is a targeted review, not an exhaustive survey of every cloth paper.

## What the current implementation establishes

The production path minimizes an implicit-Euler objective using local VBD updates. Membrane, bending, sewing and attachments participate in that objective. OGC supplies normal contact forces and conservative movement regions. The GPU path remains hybrid: GPU candidate search and local solving, with precise CPU contact geometry and movement bounds. See [the implementation contract](ogc.md).

Its robustness is valuable evidence on the tested scenes, but does not establish physical fidelity, convergence or a universal intersection guarantee. Initial separation, conservative candidate coverage, finite precision and explicitly local sewing exclusions matter. Contact forces currently depend on mesh/contact multiplicity; Coulomb friction and calibrated material response are missing. There is no common backend interface or runtime plugin system yet.

The [latest seam experiment](seam-block-refinement.md) must remain disabled. Coupling sewing participants, including membrane curvature, did not pass the combined rest-motion, release and cost checks. In its paired pull/release comparison, increasing normal iterations from 16 to 64 reduced the final motion-monitor value from about 30.5 to 13.4 micrometres while raising mean full-step time from 18.4 to 35.5 ms. These are earlier measurements, not new runs. The higher-iteration branch is not a converged oracle, and motion highpass includes legitimate acceleration.

My interpretation: insufficient convergence is a credible contributor. Contact restriction, contact-model changes and discretization remain separate possible contributors. A hotspot with no active contact means contact handling alone cannot explain every oscillation. Neither that observation nor a failed seam-only block disproves VBD as a suitable basis.

## Candidate map

| Family | Evidence and intended experiment | Main qualification |
|---|---|---|
| VBD + OGC | [VBD, 2024](https://ankachan.github.io/Projects/VertexBlockDescent/index.html) and [OGC, 2025](https://graphics.cs.utah.edu/research/projects/ogc/) support the architecture already used here. Keep an unchanged baseline and measure convergence per millisecond. | A stable finite-budget iteration is not necessarily an accurate equilibrium. Our independent implementation is not interchangeable with the paper's benchmark implementation. |
| XPBD | [XPBD, 2016](https://mmacklin.com/xpbd.pdf) expresses compliance through constraints and accumulated multipliers. Build membrane/strain, bending and integrated sewing constraints with compatible physical units. | The formulation removes the original PBD stiffness-parameter problem; finite iteration error remains. A cheaper update does not establish a cheaper collision-complete simulation. |
| XPBD with small steps | [Small Steps, 2019](https://mmacklin.com/smallsteps.pdf) motivates comparing more substeps with fewer iterations per substep. | Its example reuses collision detection across a visual frame. Dense ruffles require our own conservative coverage and refresh argument; copying this schedule blindly would be unsafe. |
| Compliant ALM / AVBD | [AVBD, 2025](https://graphics.cs.utah.edu/research/projects/avbd/) addresses large stiffness contrasts and hard constraints. Sewing is a useful first controlled experiment. | Preserve finite physical compliance. Do not turn ordinary cloth into rigid constraints or change seam strength to make the contender look stable. The linked public demos do not establish availability of the paper's large GPU implementation. |
| Directional contact regions | [Divide and Truncate, 2026](https://arxiv.org/html/2604.15513v1), especially Planar-DAT, targets unnecessary restriction of tangential motion. | The paper still reports finite-iteration energy/momentum errors and inherited OGC boundary discontinuities. Treat this as a separate contact experiment, not an established jitter cure. |
| Newton Physics | The current [VBD guide](https://newton-physics.github.io/newton/latest/solvers/vbd.html) documents cloth, DAT, friction and an experimental solver. Compliant ALM is now the default rigid-constraint path; legacy AVBD is deprecated there. | Its guide says OGC coverage is future work. Do not call this an existing OGC reference. Pin the release/commit before integration; material and sewing equivalence require validation. |
| ZOZO / PPF | The current [official repository](https://github.com/st-tech/ppf-contact-solver) provides a Rust-first implementation, FEM, strain limits and multiple compute paths. Use as an external contact/shape comparator. | The maintainers explicitly describe offline use and remaining bugs. Penetration-free completion may require rejecting an impossible setup. Main differs from the paper branch `sigasia-2024`; neither is a promised 60 Hz backend. |
| C-IPC / GIPC | [C-IPC](https://ipc-sim.github.io/C-IPC/) handles thin structures, thickness, strain limits and friction. [GIPC](https://github.com/KemengHuang/GPU_IPC) supplies a public GPU IPC implementation. | Useful independent contact references. Their different models, tolerances and solver costs prevent treating them as automatic ground truth or timing their kernels against our complete steps. |
| Box3D | The [official engine](https://github.com/erincatto/box3d) is a rigid-body system with Soft Step, CCD and SIMD. Its core is C17; samples use C++20. | A cloth implementation would still need a shell material, bending, sewing and deforming self-contact. Use its engineering ideas first; a rigid-body lattice is a different physical model. |

The Roblox connection is real: Chris Giles is affiliated with Roblox in the [AVBD paper's author information](https://graphics.cs.utah.edu/research/projects/avbd/Augmented_VBD-SIGGRAPH25.pdf). This identifies a research contribution, not proof of deployment in Roblox's production engine.

## Other research worth retaining

**Propagating tension over larger regions.** Disney's [smoothed-aggregation multigrid work](https://la.disneyresearch.com/publication/smoothed-aggregation-multigrid/) addresses difficult cloth systems and contact constraints through multilevel solving. [GPU cloth with subspace reuse, 2024](https://wanghmin.github.io/publication/lan-2024-egc/) separates large-scale strain propagation from local contact detail. [Domain-decomposed CPU cloth, 2025](https://sig25ddmpd.github.io/) also balances local and global work. These motivate testing complete material patches and coarse corrections; they do not justify assuming that enlarging our rejected sewing block will succeed.

**Accelerating the existing iteration.** [Periodic Anderson Acceleration, HPG 2026](https://diglib.eg.org/items/d6242086-2336-4f27-8b6d-c636b7620fc4) reports accelerating VBD for elastic bodies. Only its publisher abstract was accessible in this pass. A possible later experiment would validate mixed iterates against the full objective and contact feasibility, and reset history after relevant state/model changes. The reported results do not establish safety or gains for our sewn self-contacting cloth.

**Material dissipation.** Disney's [internal-friction model, 2013](https://la.disneyresearch.com/publication/modeling-and-estimation-of-internal-friction-in-cloth/) models measured hysteresis and preferred folds. This is distinct from inter-surface friction and global velocity damping. Investigate calibrated dissipation after accounting for numerical residuals; do not fit damping solely to hide a solver artifact.

**Contact forces under refinement.** [Geometric Contact Potential, SIGGRAPH 2025](https://arxiv.org/abs/2402.00719v3) derives a continuum contact potential to address spurious forces and discretization dependence. The [official toolkit](https://github.com/geometryprocessing/GCP-toolkit) says the implementation has moved into IPC Toolkit; it is not a complete cloth engine. The abstract and repository documentation were read here. Consider this model-level comparison if contact-refresh or mesh-scaling tests expose force artifacts; it changes the contact law and therefore belongs in comparison B below.

**Discretization.** [B-spline cloth, SIGGRAPH 2026](https://simulation-intelligence.github.io/BS-Cloth/) combines smooth higher-order elements with IPC and specialized quadrature/linear algebra; the project links [source code](https://github.com/Simulation-Intelligence/BS-Cloth). It changes the surface representation, so compare geometric/physical accuracy and cost rather than forcing equal vertex counts. Defer this larger change until the baseline benchmark works.

**Graphs and learning.** [HOOD, CVPR 2023](https://dolorousrtur.github.io/hood/) uses hierarchical learned graphs and targets generalization to new garments. Its published training losses do not establish a hard collision certificate for arbitrary edited seam configurations. Graph coloring and multilevel numerical solvers are also graph methods, without being learned simulators. Consider learned prediction as a later preview/initialization experiment whose output still passes an independent physical/contact check.

**Contact engineering.** Catto's [Solver2D](https://box2d.org/posts/2024/02/solver2d/), [SIMD for Collision](https://box2d.org/posts/2026/07/simd-for-collision/) and [Contact Recycling](https://box2d.org/posts/2026/10/contact-recycling/) are useful engineering references. Recycling updates separation rather than reusing an entirely stale contact; excessive tolerances can create artifacts. Deforming triangles do not have the fixed local geometry assumed for rigid bodies. Our existing conservative pair cache already reuses candidates while recomputing geometry. Any further reuse must bound deformation and feature changes, not simply copy rigid-body thresholds.

## Two comparisons, with different meanings

**A. Isolate the numerical solver.** Keep the same compiled rest geometry, masses, constitutive energy, sewing quadrature, collision law, exclusions, inputs and physical duration. Compare VBD, its controlled variants and an XPBD formulation only where matching laws have been demonstrated. Initially use zero Coulomb friction everywhere, since that is what ClothCraft currently implements. A common collider alone does not make two total objectives equivalent.

An XPBD prototype must derive compliance and quadrature weights from the same material model, check small analytic responses and converged forces, and explain any remaining mismatch. If it cannot represent a term equivalently, move that result to comparison B. Combining XPBD updates with conservative movement regions is research: constrained position updates and multiplier updates must remain consistent. A final position clamp does not establish equivalence or preserve convergence by itself.

**B. Compare complete engines as products.** Run Newton Physics and PPF with their native contact/material models, after matching density and measured directional stretch, shear and bending as closely as practical. Report different contact thickness, friction, seam formulation and unsupported features. This answers which engine gives a useful garment at a given cost, not which numerical algorithm alone is faster.

A reference should be a convergence study with smaller timesteps and tighter tolerances, supplemented by analytic fixtures and independent contact checks. Our historical `ReferenceSolver` has different collision handling and is not a collision-quality oracle for OGC. Agreement between two engines is useful evidence, not proof.

## Benchmark scene and replay contract

The owner's future dress with ruffles should be the main application scene, accompanied by small public synthetic cases. Use the current saved Garment as another regression, not the source of special rules. Keep private geometry out of published fixtures unless the owner chooses to release it.

| Scene | What it separates |
|---|---|
| Unsewn panel: free fall and hanging bend | Gravity, mass, material response and unintended global damping |
| Two panels with equal-length sewing | Basic seam closure and convergence |
| Gathered strip with unequal sewn lengths | Easing, tension and wrinkle formation |
| The same strip with nonmatching edge subdivisions | Interpolation and resolution dependence |
| Dress with one, then several ruffle layers | Sewing coupled to dense self-contact |
| Fold, twist and pull/hold/release | Separation, sliding, recoil and delayed relaxation |
| Several mesh densities and fabric stiffnesses | Whether the result depends on one file or tuning point |

Generate grab events in material coordinates with an absolute world-space target trajectory and fixed physical timestamps. Replay identical controls; do not compute each contender's target from its own drifting current shape. Test gentle and abrupt release, several pull directions and at least one region away from sewing. Test both initial draping and restart from a shared, independently validated checkpoint compatible with every contender's thickness assumptions.

For same-model tests use identical meshes. For alternate discretizations record the approximation error and degrees of freedom. Remeshing must never infer a new rest state from a draped surface. Models with stitched/welded topology or different exclusions belong in comparison B unless equivalence is established.

Record source commits, dirty-patch identity, build/driver/device, threads, scene hash, seeds, timestep, substeps, iterations, tolerances, precision and model parameters. No hidden changes of resolution, easing, stretch limit or collision range. Clear incompatible multipliers, cached contacts and prediction history between runs.

## Measurements and pass conditions

| Concern | Required evidence |
|---|---|
| Real time | Complete wall time to advance 1/60 s of physical time, including all substeps, CPU geometry, transfers, waits and retries; P50/P95/P99/max and sustained physical-time/wall-time ratio |
| Visible interaction | Separately measure window cadence and input-to-accepted-state latency; count rejected/dropped/late physical steps |
| Collisions | Fresh independent VF/EE and self-contact checks, minimum separation, continuous-path checks where available, exclusions and rejected steps; endpoint screenshots alone cannot establish safety |
| Cloth response | Area-weighted directional strain, seam gap, deformation under load, folds and remeshing sensitivity |
| Rest and release | Motion spectrum/highpass alongside recoil, travel, sliding, release delay, force residual and convergence with budget |
| Energy | Mechanical energy components, external work, modeled dissipation, solver residual work and changes caused by contact refresh/control changes |
| Cost outside the hot loop | Compilation/precomputation, cold start, peak memory and cache/contact rebuild spikes |

A 60 Hz physical target leaves 16.67 ms for all work needed to advance that interval. Six substeps must fit together, not each claim its own 16.67 ms. Mean throughput and deadline misses are separate outcomes. The render FPS counter is not the simulation clock.

Run both fixed physical settings with a budget sweep and a fixed-budget quality comparison. Increasing iterations or adding substeps is a useful tradeoff only when their complete cost is included. Time each engine separately under comparable GPU load; repeat with rotated execution order. Then replay recorded trajectories side by side at the same physical time. Concurrent execution is useful for demonstrations, not clean isolated timings.

Stored elastic energy is legitimate. After release, kinetic energy may temporarily rise as strain relaxes, so kinetic energy need not decrease every step. Even a decrease of the within-step implicit objective is not an across-step mechanical-energy certificate. Audit the balance including gravitational potential, controls, model changes and residual work. Do not force monotonicity by freezing motion, clipping velocities or reinstating the rejected prediction guard.

A contender passes only if it respects the declared collision contract, preserves the intended material/seam response, improves the quality/cost tradeoff and does not introduce sticking or delayed release. Less highpass motion on its own is insufficient. Report failing cases alongside improvements.

## Proposed implementation order

1. **CPU preparation:** consolidate existing replay/audit output into a shared report; add the public gathered-strip and release fixtures; define the scene/control exchange format and capability declarations. Implement a small common lifecycle only when connecting two working adapters. Preserve `ReferenceSolver::step` and garment formats.
2. **Complete the current diagnosis:** on small patches, compare whole-patch convergence against sewing-only coupling. Measure final residuals with bound reactions and contact-refresh changes. This decides whether solver propagation, contact restriction or the contact law should be addressed next.
3. **XPBD contender:** implement the matched material and sewing response on CPU, validate units/forces/mesh scaling, then compare substep schedules. Retain conservative collisions with an explicitly validated integration. Do not expose it as a robust garment backend before those checks pass.
4. **External comparison:** pin Newton Physics and one PPF revision; use file/process adapters initially so an evaluation does not require a large native-language port. Document model mappings and missing sewing capabilities. Separate cold import/export overhead from steady stepping, and include any recurring adapter overhead in end-to-end cost.
5. **Targeted VBD variants:** test compliant ALM if stiffness contrast dominates; test a complete patch or safeguarded coarse correction if large-scale residuals dominate; test directional contact regions if bounds dominate sliding. Change one mechanism at a time. Preserve each failing case.
6. **GPU comparison after fresh availability confirmation:** run the unchanged baseline and validated contenders sequentially on the owner's dress, Garment and public cases. Optimize the winner's measured bottlenecks. Use synchronized recordings for visual review, especially pull/release.

This sequence keeps the authoring model reusable without promising arbitrary hot swaps of solver histories. Replacing a backend must define what transfers (geometry, velocity, physical time) and what resets (multipliers, contact state, preconditioners, prediction history). No solver selector should advertise an unimplemented or incompatible option.

## Reading scope and availability

The summaries above are deliberately bounded by what was accessible:

- XPBD: retrieved paper, formulation and evaluation sections. Small Steps: retrieved paper, abstract, introduction and comparison setup.
- VBD/OGC: official project pages and prior implementation review in `docs/ogc.md`; author VBD page links Gaia and a tiny demo. No new full audit of those codebases here.
- AVBD: official project, linked demo availability, indexed primary-paper first page/introduction and author affiliation. Full PDF retrieval failed in this pass; the earlier pinned demo audit is in [contact stability research](contact-stability-research.md).
- DAT: arXiv HTML, method, integration/results and limitations. Planar-DAT is a variant of this same DAT paper, not a separate publication. Public Newton documentation was read separately; it does not by itself prove exact paper/code equivalence.
- Newton: current official guide and linked example availability. PPF: official repository README, current capabilities, limitations and paper-version distinction. C-IPC: official project; GIPC: official repository. No runtime verification here.
- Disney multigrid/internal friction, GPU subspace reuse and CPU domain decomposition: primary project pages/abstracts. CPU full-PDF retrieval failed. No complete implementation audit or unobserved benchmark claims.
- PAA: publisher search-visible abstract; direct full-page/PDF access unsuccessful. B-spline cloth: official project and repository availability. HOOD: official project and published-paper material/contact-loss description.
- Box3D: official README; Catto's three linked articles. The earlier pinned source audit is recorded separately. No third-party fork benchmark is treated as official evidence.

All linked external documentation is a snapshot of research read on this date. Pin exact code revisions before implementing adapters. Paper performance claims were not remeasured and must not be copied into ClothCraft's performance table.
