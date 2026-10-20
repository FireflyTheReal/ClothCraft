# GPU contact search — 16 October 2026

The later [draped garment pass](draped-realtime.md) adds compact CPU incidence/keys, eight-byte candidate records, scalar GPU line-search energy and reusable CPU staging. It supersedes the performance measurements below; the conservative search contract is unchanged.

The hybrid VBD backend now queries the current triangle and edge BVHs on the GPU. It no longer rebuilds the CPU temporal pair cache during ordinary GPU steps. Exact closest features, feasible regions and OGC movement balls still use the existing f64 CPU implementation. Local seam exclusions and contact energy definitions are unchanged; VBD energy assembly retains its existing GPU implementation and sparse f64 CPU path. Mesh resolution, materials, time step and iteration count are unchanged.

## Code and ownership

- `ogc/filter.rs` is the graphics-independent boundary: a spatial query returns primitive IDs, never force coefficients or movement radii.
- `ogc/bvh.rs` exposes preorder escape indices. The shader walks a subtree without a per-thread stack; skipping a node jumps past its entire subtree.
- `gpu_vbd/contact_bvh.wgsl` refits moving node boxes and queries both VF directions and unordered EE pairs. It omits fixed/fixed, shared-vertex and explicitly excluded local sewing pairs. The CPU repeats the same adjacency checks.
- `gpu_vbd/contact_geometry.wgsl` rejects distant boxes and separated primitive projections. Its approximate closest points choose axes only.
- `gpu_vbd/contact_filter/` manages resident topology, exclusion lists and fixed avatar data, moving-position uploads, buffer growth and readback. A world identity invalidates static data after topology/avatar replacement. The search origin remains fixed for that world; the VBD solver retains its own translated frame.
- `ogc/world.rs` evaluates every surviving pair in f64 and produces the same canonical contact set and per-vertex minima. It validates positions and nondegenerate faces before GPU work. The accelerated scan does not read stale CPU boxes; a CPU fallback refits all trees before using its temporal cache.

Each moving BVH node is refitted by one cooperative workgroup over its primitive interval. This performs O(n log n) box-union work, but each node is independent: there are no cross-workgroup atomics, parent/child races or per-level CPU dispatches. The query follows the refit in the same compute pass. Fixed nodes are unchanged.

The VBD packer consumes the detector's contact incidence directly, including the same ordering and any grab incidence. It no longer builds another heap-allocated list for every vertex. Static material terms and their incidence stay resident; a virtual concatenation in WGSL preserves the previous per-lane summation order while uploading only the dynamic tail.

## Conservative rejection

For any nonzero axis `a`, the separation of the two primitives' projected intervals divided by `|a|` is a lower bound on their Euclidean distance. The intervals include every vertex of the edge or triangle. Therefore an inaccurate approximate closest point only selects a worse axis; it cannot justify rejecting a pair by its approximate distance alone.

The uploaded coordinates are translated before conversion to f32. Let `M` bound their magnitude and the query radius, and `W` bound the original world-coordinate magnitude. The padding is `64 eps32 M + 64 eps64 W`. Box gaps are reduced by this padding and the query radius is increased by it. Projected intervals use an additional `4 padding |a|_1` error allowance. This deliberately admits extra pairs around the cutoff. Degenerate estimates and very small axes survive for f64 evaluation; divisions used to choose axes have explicit guards. Inputs outside the supported numeric range use the complete CPU detector.

These are conservative floating-point filters, not exact-arithmetic geometric predicates. Existing OGC assumptions and the local sewing exclusions still apply. A GPU search does not repair an initially intersecting garment.

## Capacity and fallback

The output counter reports every retained pair. If storage fills, an overflow flag is set, the partial result is discarded, storage grows and the complete query is repeated. The flag prevents even a counter wrap from making an incomplete result appear valid. Device storage/dispatch limits and unsupported coordinate scales select the complete CPU path; they never truncate the contact set.

Topology, fixed positions and fixed BVH nodes remain resident. Only moving positions and query parameters are uploaded each step. Buffers grow when necessary; dispatch counts and mapped byte ranges always use the current scene. Shrinking or replacing a garment does not expose old records. Readback waits target their own submission index, rather than also waiting for a later renderer submission on the shared device.

## Measurements

`contact_profile.filter_ms` includes packing, submission, synchronization and readback. Its prepare/dispatch/decode fields are wall-clock breakdowns, not GPU timestamps. `spatial_queries` counts GPU primitive queries, `filter_pairs` counts survivors and `spatial_retries` reports capacity retries. CPU `candidate_pairs`, `candidate_blocks` and cache rebuild counts are zero when that cache is bypassed. `aabb_rejections` counts only the subsequent CPU box tests, not all GPU rejections.

`gpu_compute_ms` remains the VBD compute timestamp. It does not include the new search. Complete `step_ms` includes the current seam-gap and stretch diagnostics. Compare that value and actual simulation-clock progress in the native window when judging realtime performance.

## Current results and remaining limit

On RTX 3090 / Ryzen 7 2700X, with two CPU threads, the private 6,731-vertex / 12,682-triangle garment retains its 1/60 s step and 16 iterations. A consecutive 240-step comparison changed the mean full step from **35.22 to 16.22 ms**, and P95 from **52.68 to 24.97 ms**. Desktop load and GPU clocks were uncontrolled; an earlier baseline averaged 25.93 ms. These are local measurements, not a hardware-independent speed claim.

The final 1,800-step run completed 30 simulated seconds with **17.93 ms mean / 22.53 ms P95**. Its final positions, velocities and time match the previous f64-contact baseline exactly. There were **zero CPU pair-cache rebuilds** and five GPU capacity retries. GPU submission/wait spikes remain: the largest complete step was 45.17 ms, so removing the cache does not eliminate all latency peaks.

**Sustained 60 simulation steps/s is not achieved across the garment's full trajectory.** The final native measurements delivered **54.03 steps/s from the saved arrangement and 45.45 steps/s from the 30 s drape**, while rendering stayed near 60 FPS. An intermediate build measured 59.19 steps/s on the initial scene. Do not report renderer FPS or the best short run as proof of 60 physical steps/s everywhere. Remaining costs include exact f64 contact processing, VBD solves, precision-limited vertices and synchronization under desktop load.

The diagnostic oracle compared complete CPU contact detection against the GPU path for 120 initial and 120 draped steps, with exact agreement in contacts, incidence, near/active counts and original radii. Final seam gap is 4.157 mm; positive extension P95 is 5.528%, maximum 29.688%. These quality metrics are unchanged, not a claim that the garment is production-ready. [Machine-readable evidence](gpu-contact-validation.json).

Validation passed: 138 tests without desktop features, 64 tests in the selected desktop targets (including shared tests), six opt-in GPU tests, six template validations and Clippy with warnings denied in both configurations. A fresh CPU probe of the final state passes initial intersection validation and reports a positive minimum separation of 1.44025e-7 m under the existing local seam exclusions. The two final native performance acceptance runs fail; correctness checks passing does not establish the 60-step target.

## Reproduce

```powershell
# Explicit GPU opt-in. Keep the project's dt, mesh and iteration settings.
.\target\release\cloth-fit.exe --project path/to/garment.json --gpu-sim --steps 240 --threads 2 --output output/gpu-search
# Same VBD backend with the complete CPU contact search.
.\target\release\cloth-fit.exe --project path/to/garment.json --gpu-sim --cpu-contact-filter --steps 240 --threads 2 --output output/cpu-search
# Diagnostic oracle; its extra CPU detection is included in step_ms, so this is not a performance run.
.\target\release\cloth-fit.exe --project path/to/garment.json --gpu-sim --check-gpu-contacts --steps 120 --threads 2 --output output/contact-oracle
# Actual native-window simulation cadence, one second warm-up + five measured.
.\target\release\cloth-fit.exe --project path/to/garment.json --steps 0 --threads 2 --ui-perf-smoke --output output/window-search
# GPU equality/capacity/motion/precision regressions, never part of ordinary CI.
cargo test --locked --release -j 2 --test gpu_contact_filter --test gpu_motion --test gpu_precision -- --ignored --test-threads=1
```

The default CPU suite validates the BVH escape traversal against exhaustive queries, the temporal fallback cache and the WGSL modules without running a GPU. The opt-in contact test compares exact radii, minimum distance, canonical contacts, incidence and active/near counts across deformations, scale changes, large translations, buffer growth/shrink and CPU fallback. User garment files remain under ignored local output paths.
