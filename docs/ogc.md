# Offset Geometric Contact and VBD

**19 October EE investigation:** [endpoint activation diagnosis and CPU experiment](edge-transition.md). The experimental interior potential is off by default and changes the contact law; it must not be described as unchanged OGC or an accepted jitter correction. Safety bounds remain identical.

**18 October CPU update:** independent edge-pair closest geometry uses four-lane f64 AVX when available, with the scalar oracle and automatic fallback. Both cached CPU candidates and hybrid primitive candidates share this path. No contact recycling, tolerance, exclusion, force law or movement-bound change. Read [CPU evidence and jitter diagnosis](cpu-contacts-and-jitter.md); the measured 9.8–10.6% detector saving is not a new GPU or full-step result.

**Settled-motion rollback (16 October):** the objective-based prediction rejection was removed after interactive testing showed jerky relaxation and impaired sliding. The previous adaptive prediction and local line search are restored, with original OGC balls and collision processing unchanged. Passive `motion` diagnostics remain. Jitter is unresolved; see [the rejected experiment](settled-jitter.md).

**16 October GPU search update:** the interactive backend now refits and queries the BVHs on GPU, with conservative box/projection rejection, local exclusion lists and overflow retries. Exact closest features, feasible regions and movement bounds remain f64 CPU work; contact energy definitions and their existing GPU/sparse CPU assembly are unchanged. The temporal leaf-pair cache described below remains the complete CPU path and fallback. Read [the current GPU contract and validation](gpu-contact-search.md). This is still a hybrid pipeline.

Independent Rust implementation of the normal-contact method described by Chen et al., **Offset Geometric Contact**, SIGGRAPH 2025. Primary source: [project and paper](https://graphics.cs.utah.edu/research/projects/ogc/). The PDF was read, particularly sections 3.2–3.7 and Algorithms 1–4. The [official IPC Toolkit tutorial](https://github.com/ipc-sim/ipc-toolkit/blob/main/docs/source/tutorials/ogc.rst) was also consulted; no C++ code was incorporated.

## Implemented scope

- `ogc/topology.rs`: face, edge and vertex adjacency and canonical IDs. Manifold edges; open boundaries allowed.
- `ogc/geometry.rs`: vertex/edge feasible regions (Equations 8–10 and 14–15), closest primitives and initial-intersection diagnostics.
- `ogc/bvh.rs`: separate cloth/avatar roots, moving-primitive refit and individual-box filtering within leaves. Queries have no silent candidate-count limit.
- `ogc/candidates.rs`: conservative pair-ID cache. Search at `R + 2s + epsilon`, with upward-rounded `R >= rq` and `s = 2 × OGC radius`. Reuse only while every moving vertex remains within `s` of its cache center and `rq <= R`. Convexity and the triangle inequality bound primitive-distance changes by `2s`. Distances, feasible regions, forces and bounds are **recomputed at every detection**. Fixed geometry cannot change without reconstruction. Two independent CPU tasks merge minima and canonical contacts.
- `ogc/world.rs`: VF in **both directions**, EE, self-contact, deduplication on the same primitive, and conservative distances propagated to every primitive vertex. Distances used for bounds are evaluated even when feasible-region or parallel-edge filters reject a force.
- `ogc/potential.rs`: quadratic then logarithmic activation, exact AD derivatives and a polynomial nearly-parallel-edge filter. Each canonical primitive interaction is integrated once; shared VV/VE contributions from VF and EE traversals are merged.
- Spherical bounds `0.45 * min(dVF, dEE, dFV)` around positions at the **last detection**, not the last iteration. The OGC projection limits each local proposal before its energy check.
- `vbd.rs`: implicit Euler with the same anisotropic membrane, bending and sewing as the reference. Local 3×3 Newton, exact Hessian regularized to SPD, local descent search. Coloring includes contacts and mouse attachments: a color shares no element.
- `gpu_vbd.rs`, `vbd.wgsl`: GPU energies, forces and VBD iterations with approximate positive local Hessians. The gradient retains EE-filter derivatives. `--check-gpu-gradients` compares GPU energies/forces against f64 AD at identical f32-rounded geometry.

## Prediction and conservative candidates

**Earlier contact-activation correction:** feasible interactions are retained up to `rq`, even when their potential is zero at detection. Only `r` activates a force. With once-per-step detection, this allows activation during the solve instead of waiting for the next step. It does not guarantee exhaustive force coverage after large movement or a feature change. Coloring and local search include dormant terms; bounds remain unchanged. Canonical sorting/deduplication replaces tree insertion. GPU coordinates are translated near the garment and use position differences. See [diagnosis, measurements and limits](jitter.md). `contacts` counts positive-energy interactions at detection; `contact_terms` counts all retained interactions. Residual jitter remains unresolved.

Candidate construction traverses **pairs of tree nodes**: point/face for VF and edge/edge for EE. The point BVH also has moving/fixed roots; only moving primitives are refitted. Fixed/fixed and distant-box pairs are rejected in groups; reciprocal VF and EE remain. EE traversal visits each unordered pair once and preserves historical operand order for geometry calculations. Tests compare sets with exhaustive queries after deformation. See [performance research](performance-research.md).

CPU and GPU steps start with **adaptive VBD initialization (§3.7)**, clipped to the **initial detection bounds**. Collective prediction preserves free fall; acceleration estimated from the last accepted step avoids repeatedly injecting free fall into supported vertices. The objective retains full gravity. The estimate accounts for damping and its history is invalidated after editing/reload. It respects fixed vertices and f32 margins; centers do not move between prediction and solve. See [stretch diagnosis and correction](adaptive-initialization.md). Opt-in `tests/gpu_motion.rs` covers free fall, buffer growth, high-speed bounds and supported cloth over several seconds.

Candidate construction uses a lower bound on AABB distance. Exact VF/EE evaluation remains part of every detection, including the first step. False positives are allowed, but no nearby pair may be removed. `tests/ogc_bvh.rs` compares the bound with geometric distances; `tests/ogc_cache.rs` compares cached results with fresh reconstruction.

Since the 15 October CPU pass, cached pairs also pass a **current** AABB filter before exact geometry. Only boxes farther than the query radius, with outward rounding allowance, are eliminated. Potentially closer distances are always evaluated, including dormant terms and infeasible regions that bound movement. `aabb_rejections` counts avoided exact evaluations. Coloring reuses incidence with unchanged groups/vertex order. See [local proof, tests and limits](research-2026-10-15.md#implemented-safe-cost-reductions). Since 16 October, the cache movement budget is two OGC radii and its search envelope grows accordingly; physical bounds remain unchanged. See [measurements and the two-moving-primitive regression](precision-stall.md#candidate-cache-cost).

Since the 16 October continuity pass, the temporal cache stores **BVH leaf pairs**, without expanding every primitive pair during reconstruction. Each detection filters current leaf boxes, then current primitive boxes, applies the same local exclusions and recomputes exact f64 geometry. Coverage `R + 2s` and reuse conditions are unchanged. `candidate_blocks` counts leaf pairs; `candidate_pairs` counts their possibilities before exclusions/current filtering. See [coverage and measurements](realtime-follow-up.md#cost-reductions).

## Logarithmic join coefficient

Equation 19 of the PDF displays `k' = tau * k * (tau-r)^2`. This coefficient does not satisfy the stated derivative continuity: `g'_quadratic(tau) = k*(tau-r)`, `g'_log(tau) = -k'/tau`. The code uses the coefficient **derived from the join conditions**, `k' = k*tau*(r-tau)`. With `tau=r/2`, both second derivatives equal `k`. The logarithm uses the dimensionless ratio `d/tau`. Finite differences and both sides of the join test this choice. At `d=r`, the law is C1, not C2; do not claim C2 everywhere.

## Sparse directional extension (19 October)

The VBD constructor now enables a sparse extension for pairs closer than
`query_radius / 64`: verified fixed separating half-spaces replace that pair's
isotropic restriction. Other pair bounds and the query-radius envelope remain.
The hybrid path solves these vertices in f64 on CPU, with the unchanged
implicit objective. Raw `OgcWorld` defaults to the historical isotropic mode.
The ball-only descriptions above remain the baseline and fallback, not the
complete feasible region of a directional vertex. Read the
[safety argument, integration, laboratory and limitations](directional-contact.md)
before changing clipping, refreshing regions, or interpreting stall reports.

## Contract and limitations

OGC prevents a new intersection from a separated state for pairs included in detection, under the paper's geometric assumptions. It does not repair existing intersections. Construction rejects detected initial intersections; step errors are transactional. The initial diagnostic uses floating-point tolerances, not certified exact geometric predicates.

Seams retain two distinct boundaries. Vertices joined by the same sewing quadrature are therefore excluded **locally** from the corresponding collisions. Whole panels remain in self-contact. The guarantee does not cover excluded pairs. Sewn topology or dedicated thickness handling is still needed for industrial sewing semantics.

Normal contacts are two-sided, including the mannequin. We do not use the unilateral volumetric variant in §3.9. There is no Coulomb friction, animated avatar, repair of existing intersections or physical yarn model. Fabric parameters are uncalibrated.

Offset radius (m) is distinct from material thickness. `stiffness` is **N/m per pair**, not the historical collider pressure coefficient in N/m³. Its effect depends on mesh resolution and contact multiplicity; resolution convergence is not claimed.

Contact sets remain fixed between detections. CPU detection refreshes when enough vertices reach their bounds. The hybrid backend detects **once per step**, retaining the same centers/radii through all GPU iterations. This preserves bounds but can slow sliding and relaxation. GPU radii subtract a rounding margin; precision-limited vertices remain in f64 and are excluded from GPU solve dispatches. Two local f64 sweeps use the same objective and original bounds. Numerically unresolved separation (≤ 0.1 nm) can still stop a step. See [stall reproduction and checks](realtime-follow-up.md).

This remains hybrid: BVH refit/query runs on GPU, while exact f64 contact geometry, movement balls, coloring, dynamic packing and readback processing remain CPU work. GPU allocations, bind groups, readbacks and queries are reused/grown as needed; dynamic data still transfers per step. Static material terms and incidence remain resident; only contact/grab tails upload each step, except after reconstruction or buffer growth. Material terms/incidence compile once per garment, invalidated by retained Arc identity. A 32-thread workgroup assembles each vertex's contributions; reductions are synchronized and coloring is unchanged. Parallel sums change f32 rounding, so trajectories are not bitwise identical to the historical serial kernel.

A fixed VBD budget does not guarantee minimizer convergence at each step. CPU residuals are reported; uncomputed GPU residuals are `null`. `step_ms` is complete wall time. On GPU, `detection_ms` includes detection/coloring; `solve_ms` includes preparation, transfers and GPU waits. None is a pure GPU timestamp.

`gpu_compute_ms`, when available, is a hardware timestamp of the VBD compute pass. It excludes GPU contact search, CPU work and transfers. `contact_profile.filter_dispatch_ms` includes contact submission/wait/readback and is a wall-clock measurement. The historical OGC validation used a 3 mm radius, contact stiffness 100,000 N/m per pair, seam stiffness 50,000 N/m², eight iterations and a 1/60 s step. The current private-garment benchmark preserves its 16 iterations. These settings are not a material calibration.

`--ui-interaction-smoke` injects pointer events into a real egui window, checks that a triangle was grabbed and an attached step accepted, writes `interaction.json`, captures and closes the window. It explicitly requires the GPU and remains outside ordinary tests.

## Realtime scheduling

**Precision-limited vertices (16 October):** the hybrid backend retains vertices whose ball is dominated by the f32 roundoff margin in double precision. Two sparse CPU local-minimization sweeps use the same objective and original OGC balls after GPU readback. No radius or contact is relaxed. `precision_vertices` / `precision_ms` report the work. See [the stall diagnosis, invariant and tests](precision-stall.md).

The [Turbo pass](turbo-perf.md) separately measures physical throughput and actual window cadence. `atelier/worker.rs` owns current state and a fixed-step clock independent of drawing. Catch-up is limited to four steps after a duration spike, without increasing dt or reducing iterations. Newest controls replace older ones; only one result waits for the UI, and generation numbers prevent old meshes from replacing new ones. `--ui-perf-smoke` measures five seconds after one second of warm-up using the unique egui frame counter and writes `ui-perf.json`.

## Run

```powershell
# OGC/VBD reference without GPU
cargo run --release --locked --no-default-features -j 2 --bin cloth-fit -- --steps 30 --mesh-mm 30 --output output/ogc-cpu

# Explicit GPU computation with full measurements
cargo run --release --locked -j 2 --bin cloth-fit -- --gpu-sim --steps 120 --mesh-mm 30 --output output/ogc-gpu

# Numerical GPU validation (expensive, opt-in)
cargo run --release --locked -j 2 --bin cloth-fit -- --gpu-sim --check-gpu-gradients --steps 4 --mesh-mm 50 --output output/ogc-check

# Workspace: asynchronous simulation, left-button cloth grabbing
cargo run --release --locked -j 2 --bin cloth-fit -- --steps 0 --view --output output/atelier
```

`--reference` retains historical Newton/PCG with its discrete contact penalty; it is distinct from OGC. `--gpu-benchmark` measures **rendering**; `--gpu-sim` runs hybrid physics. Ordinary tests remain CPU-only and also validate WGSL through Naga.

## Finite-primitive ablation

The [CPU finite-distance experiment](primitive-contact.md) removes feature-refresh energy jumps by retaining complete primitive pairs and evaluating finite distances during the solve. It keeps geometric movement bounds, enlarges force coverage, and is off by default. It fails the planar tangential-force gate; it is not a promoted OGC implementation change or a full IPC/GCP backend.
