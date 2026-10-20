# Research and decisions — 14 October 2026

The [long-tail follow-up](contact-long-tail.md) exposes later convergence failures in all four constrained-search protocols. Passive backtracking traces identify actual EE clearance violations and unresolved swept checks, not merely an overly small OGC prebound. An opt-in search-path repair rescues four isolated failures and passes four short protocols, but a new longer trajectory fails at step 479. A final [moving-plane certificate](moving-contact-separator.md) rescues that isolated step while retaining clearance; long-run validation is absent. Neither experiment changes the editor.

The [geometrically constrained residual-search experiment](constrained-contact-search.md) records the independently implemented null-space/QR method, actual LAPACK documentation consulted, independent numerical references, four passing synthetic release protocols and the unresolved scaling/dress limits. It is a CPU laboratory option, not an editor backend.

**20 October — contact rank and complementarity.** [Independent QR search and full test matrix](contact-rank.md). LAPACK complete-orthogonal-factorization documentation informs the numerical approach; this Rust implementation has its own rank criterion and is not a port. One full eased moving-release protocol passes; three still fail on nonnegative reaction transitions. This does not establish garment-scale robustness or performance.

**20 October — integrated discrete-force trial:** [coupled pilot and negative gates](discrete-contact-solve.md). An independent CPU implementation advances two formerly energy-increasing synthetic steps with decreasing energy, using distinct force/geometry rows and actual nonlinear residuals. Full sewn pulling protocols still fail on active-contact degeneracy. This is not a robust integrator, paper reproduction, production fix or realtime result. Preserve the unchanged editor and the complete failed traces.

**20 October — discrete contact work / invariant gradients:** [source review, independent VF/EE kernel and offline audit](discrete-contact-work.md). Hesch–Betsch NTS/mortar papers and the Gautam–Sauer convergence warning inform separate work, momentum and solve-quality gates. An invariant discrete force row passes local tests and removes positive work on two frozen reaction captures; a frozen-witness shortcut fails frictionless sliding. No new time integrator or corrected cloth trajectory is claimed. Endpoint geometry and discrete force derivatives must remain distinct in the next solver experiment.

**20 October — sewn self-contact / discrete work:** [combined fixture, boundary pivots and energy accounting](sewn-self-contact.md). An independent active-set improvement resolves a captured feasible search QP and completes a stalled release step. The stronger full protocols expose energy increases from positive discrete contact work despite small force residuals; moving release fails the full gate. Preserve those negative results. This is an independent CPU research implementation, not an installed backend or a paper-reproduction claim.

**20 October — redundant-contact follow-up:** [independent dual blocks and exact Schur reuse](independent-contact-blocks.md). Conditional independent blocks keep every inequality and reduce dual work without contact softening. Cached/uncached trajectories match exactly; repeated moving/held-release gates pass on the eight-vertex deforming fixture. Moving-release cost improves, held release does not. This is an independent implementation and CPU research result, not a cited paper implementation, production jitter fix or garment benchmark.

**20 October — deforming-contact follow-up:** [coupled VF/EE, swept certificates and path correction](deforming-contact.md). The new two-layer scene exposes artificial blocking that energy-only diagnostics miss. Accumulated path corrections pass moving/held release at a higher nonlinear budget; rank-deficient dual solves make the hard case too expensive. Research-only, with original OGC protections, no production jitter or whole-dress performance claim.

**20 October — coupled contact/material follow-up:** [sewn-patch results](coupled-contact.md). Existing cloth energies are now exercised with a dense joint inequality solve. Active-block dual updates preserve matched trajectories while accelerating small matching/eased-seam pull/release fixtures. This is hard floor contact with OGC safeguards, not yet coupled deforming self-contact or a production backend.

**20 October — unilateral-contact experiment:** [local QP and release gates](unilateral-contact.md). Independent hard-inequality kernel with full SPD metric and explicit KKT diagnostics; sixteen planar point/rigid-wire release runs pass using finite-geometry constraint generation, while imposing every tangent inequality fails convergence. This changes the contact model and is research-only. No complete AVBD/XPBD backend, deforming-cloth validation or production jitter correction is claimed.

**Update after the initial review:** CPU VBD, OGC VF/EE normal contact and WGSL VBD compute are implemented and have run on GPU. Read [OGC](ogc.md) and [validation](ogc-validation.md). The material formulations below remain in use. References to a future backend or no GPU launch describe the initial phase, not the current implementation. Later AVBD-related CPU experiments live in the laboratory; no AVBD/PD/IPC editor backend has been added.

## Initial audit

The initial working directory was empty, including hidden files: no `Cargo.toml`, Python/Warp/XPBD code, tests, Git history or uncommitted changes to continue. Rust 1.97.1 and Cargo 1.97.1 were available.

Scope of the initial work: an executable start to Phase 1. The sources below were a first review of official pages, publications and licenses, not a complete reproduction of the papers or their benchmarks.

## Implemented energy

All rest positions `u` come from the 2D pattern, in metres. For each triangle:

```
Dm = [u1-u0, u2-u0]          Ds = [x1-x0, x2-x0]
F = Ds inverse(Dm)           A = abs(det(Dm))/2
w = (cos(a), sin(a))         f = (-sin(a), cos(a))
ew = (|F w|²-1)/2            ef = (|F f|²-1)/2
g = (F w) dot (F f)
E_membrane = A [phi(ew, kw) + phi(ef, kf) + ks g²/2]
```

Compression softening `r` and transition width `d` enter:

```
p(e) = (e + sqrt(e²+d²) - d)/2
phi(e,k) = k/2 [r e² + (1-r) p(e)²]
```

This smooth energy is zero and stationary at rest and remains objective under 3D rotation. `kw`, `kf`, `ks` are in N/m. `kw` and `kf` are asymptotic tensile coefficients, not exact tangent moduli at zero strain (smoothing changes them). There is no fitted Poisson coupling or measured law. This is an independent Green-strain model, **not an exact Baraff–Witkin/Kim implementation**. Indefinite Hessians remain possible.

For an internal edge with rest length `l`, adjacent areas `A1,A2`, consistent unit normals `n1,n2` and rest transverse direction `t`:

```
B(t) = Bw (t dot w)² + Bf (t dot f)²
E_bend = [3 l²/(A1+A2)] B(t) (1 - n1 dot n2)
```

The implementation evaluates the final factor as `|n1-n2|²/2` to avoid cancellation in `1-cos(theta)` near zero angle. Derivatives remain those of the energy, without clamping.

`Bw,Bf` are in N·m. The pattern is flat, so the rest angle is zero. Energy is rigid-motion invariant and quadratic at small angles. This directional interpolation is not the complete orthotropic bending tensor; the bounded cosine energy does not prevent complete folding. Compare it with a better-conditioned shell/hinge formulation before calibration and fold benchmarks.

Nodal mass is `density * sum(A/3)`. The sphere penalty is `k_contact * A_vertex * min(distance-R-thickness/2,0)²/2`, with `k_contact` in N/m³. Area weighting avoids arbitrarily multiplying total stiffness under refinement. Initial penetrations are counted and enter this penalty; the exact sphere center is rejected. There is no general intersection repair.

## Initial reference solver

Minimize the incremental energy:

```
I(x) = sum_i [ m_i/(2 h²) |x_i-x_old_i-h v_old_i|²
             + gamma m_i/(2 h) |x_i-x_old_i|² - m_i gravity dot x_i ]
       + E_membrane + E_bend + E_contact
```

Newton uses exact gradients/Hessians through local second-order automatic differentiation. Rayon parallelizes elements; reductions follow mesh order. PCG uses element-Hessian actions and a diagonal preconditioner. Fixed DOFs are eliminated from directions. Negative curvature or an inadmissible direction triggers increasing diagonal regularization; Armijo then checks decrease of the **unregularized** energy. Damping belongs to the objective rather than being added after convergence.

The residual is RMS force over free DOFs, compared with `max(abs_tol, rel_tol * initial_residual)`. Iteration-limited results are not marked converged. A step error preserves the previous state. Near-degenerate sampled geometry is rejected; this does not imply CCD.

## Sources, rationale and next steps

| Primary source consulted | Decision at the initial review |
|---|---|
| [Baraff & Witkin, 1998](https://people.csail.mit.edu/bmcutler/gaudi/baraff_witkin.pdf) | Historical reference for implicit integration of stiff cloth. No code reused. |
| [Kim, FEM Baraff–Witkin Cloth, 2020](https://www.tkim.graphics/FEMBW/) | Anisotropic formulation and indefinite Hessians motivate objectivity/derivative tests. Our Green-strain law is distinct. |
| [Stable Discrete Bending, 2023](https://wangzhendong619.github.io/papers/Wang-2023-SDB.pdf) | Listing/abstract located; full PDF download through the web tool failed. Investigate degeneracy and spectral projection; not implemented. |
| [Data-Driven Elastic Models for Cloth, 2011](https://wanghmin.github.io/publication/wang-2011-dde/Wang-2011-DDE.pdf) | Experimental calibration and nonlinear response: current demonstration parameters must not be described as measured fabric. |
| [Projective Dynamics, 2014](https://www.projectivedynamics.org/projectivedynamics.pdf) | Reference identified; full PDF unavailable through the tool. A future PD solver must specify which energies its projections support. |
| [VBD, 2024](https://ankachan.github.io/Projects/VertexBlockDescent/index.html), [Gaia](https://github.com/AnkaChan/Gaia) | At the initial review, a future block-descent backend sharing the reference model/metrics; compare equal residuals and constitutive laws. VBD was implemented subsequently. |
| [AVBD, 2025](https://graphics.cs.utah.edu/research/projects/avbd/) | Investigate augmentation for stiff sewing/constraints. Distinguish augmented formulation, contact and material model. |
| [C-IPC, 2021](https://ipc-sim.github.io/C-IPC/) | Barriers, thickness and ACCD are related components. Our sphere penalty provides none of their guarantees. |
| [ZOZO PPF](https://github.com/st-tech/ppf-contact-solver) | Repository/documentation identified for an accurate path; no integration or numerical comparison performed. |
| [GIPC](https://github.com/KemengHuang/GPU_IPC) | GPU IPC architecture to study after CPU contact validation; no CUDA execution. |
| [OGC, 2025](https://graphics.cs.utah.edu/research/projects/ogc/) | Offset-face formulation and local movement bounds, not a simple replacement for an IPC barrier. No official code link was identified on the consulted page; do not assume availability. |
| [BS-Cloth, SIGGRAPH 2026](https://github.com/Simulation-Intelligence/BS-Cloth) | Repository identified for a future B-spline discretization. A spatial basis change is distinct from a solver switch. |
| [Multilevel Additive Schwarz, 2022](https://wanghmin.github.io/publication/wu-2022-gbm/) | Candidate preconditioning for large stiff systems. The initial reference uses Jacobi only. |
| [Automatic Digital Garment Initialization, 2024](https://wanghmin.github.io/publication/liu-2024-adg/) | Initial panel arrangement and sewing relationships influence assembly. Prioritize after the two-sewn-panel example. |
| [Genesis, Simulator](https://genesis-world.readthedocs.io/en/latest/api_reference/engine/simulator.html) | Architectural reference separating solvers and coupling. No Genesis component is integrated. |

ARCSim/remeshing still needs deeper investigation: this initial review claims no source-code reading or license validation. Remeshing must transfer material configuration, mass and state without silently changing the rest pattern.

## Licenses and reuse

The initial energies, solver, tests and renderer were written in this repository. No third-party research code was copied during that initial pass. The licenses below were **read at their URLs** on the date of that review; internal repository dependencies were not audited. Later integrations have their own provenance records.

| Repository | Root license read |
|---|---|
| [Gaia/LICENSE](https://raw.githubusercontent.com/AnkaChan/Gaia/main/LICENSE) | Apache-2.0 |
| [ppf-contact-solver/LICENSE](https://raw.githubusercontent.com/st-tech/ppf-contact-solver/main/LICENSE) | Apache-2.0 |
| [GPU_IPC/LICENSE](https://raw.githubusercontent.com/KemengHuang/GPU_IPC/main/LICENSE) | MPL-2.0 |
| [BS-Cloth/LICENSE](https://raw.githubusercontent.com/Simulation-Intelligence/BS-Cloth/main/LICENSE) | Apache-2.0 |

Links on `main` move: pin a commit and recheck files/notices before reuse. C-IPC, AVBD demos, ARCSim and OGC had no verified reuse authorization in this initial review. Reading a paper does not grant a general license to its code or data.

The GUI uses [eframe 0.36.2](https://docs.rs/eframe/0.36.2/eframe/) (egui/winit/wgpu), with an original 3D pipeline. Resolved versions and the complete dependency list are in `Cargo.lock`. At the time of the initial work, there was no CUDA/compute backend and no graphics/GPU launch; later GPU work is documented separately.

Downloaded dependency `Cargo.toml` metadata was also inspected: eframe/wgpu, glam, rayon, serde/serde_json, clap and naga declare `MIT OR Apache-2.0`; tracing/tracing-subscriber `MIT`; bytemuck `Zlib OR Apache-2.0 OR MIT`. This records direct dependencies and wgpu, not an exhaustive legal audit of transitive dependencies or bundled resources.

## Historical extension sequence

**20 October 2026 update:** the sequence below is historical. The parallel-contact study reread [GCP §5.3 and Appendix A](https://arxiv.org/html/2402.00719v3). Only Appendix A feature-transition factors have been independently implemented and tested in Rust. The full GCP potential, directional factors, two-sided cloth adaptation and backend are not implemented. See [measured scope and rejected alternative](parallel-contact-gates.md).

1. Visually inspect the desktop when the GPU is available; consolidate the CPU model and compression/bending scenes.
2. Two patterns and sewing: subsequently implemented in the CPU CLI/library, with quadrature across different discretizations and force-balance tests. See [targeted research](research-patterns.md) and [format](template-format.md). Editor integration and general contours were still pending at this historical stage and have since been added.
3. CPU BVH, VF/EE distances, topological exclusions and initial-intersection detection, then persistent friction and consistent CCD/barriers. Validate before multilayer work.
4. CPU VBD using the same energies; graph coloring and equal-residual comparison. AVBD and PD are separate implementations with explicit assumptions.
5. Membrane then bending WGSL compute, f32/f64 validation, profiling/transfers; GPU contact and MAS afterward. No part of this phase had run on GPU at the initial review.

# 20 October 2026 — contact feature hierarchy audit

The [four-family snapshot audit](feature-hierarchy.md) implements direct
VF/EE/VE/VV candidate queries and geometric lower-feature handoff checks.
It preserves production OGC contacts and bounds. Read-only dress and prior
release snapshots have no missing geometric handoffs at the tested radii;
this is not a complete contact potential or a jitter correction.
The [GCP formulation](https://arxiv.org/html/2402.00719v3), §4 and §5.2.1,
derives its exterior criterion for oriented volume boundaries. Open,
two-sided cloth still needs an explicit directional convention, quadrature
and fully differentiated solver assembly. Existing Appendix A mollifiers
and this candidate set do not supply those missing pieces.

## Two-sided force trial

The [new independent trial](two-sided-contact-trial.md) adds differentiated
local-minimum directional factors and full VF/EE/VE/VV pair energies.
It omits the oriented-volume exterior condition and is not full GCP.
Flat-point and full-patch scans expose force ripples; guarded pull/release
fails fluidity despite decreasing mechanical energy. Keep production unchanged.
The normalized local field in [Larionov et al. 2021](https://elrnv.com/fcses_tog21.pdf),
§4.1, is a subsequent research lead; §7 excludes self-contact. No MLS backend
has been implemented.

## Local surface field trial

The [normalized local plane field](local-surface-contact.md) is now implemented
as a CPU research kernel, with all-geometry derivative tests and guarded
pull/release. Its six planar trajectories match the analytical control.
It is not a complete MLS contact backend: mixed sheets create a false surface,
and a read-only dress neighborhood audit measures up to 1.665 mm gap error.
Original geometric guards and production forces remain unchanged. The next
gate is surface ownership and geometric conservatism, not a lower damping value.
