# Performance baseline — Round 1 (measurement only)

**Status.** Measured baseline for the standing performance task force. Round 1
changed **no source file**; it only measures and records. Every optimisation
claim later must be a same-scene, same-settings before/after in this table.

**Revision pin.** All CPU, allocation and build numbers below come from
`16f6fc4` ("Make the interface plain English, widen the CLO3D register, ship
DRAW-01"). The working tree was dirty with in-flight UI translations while this
ran, so builds used a pinned `git archive 16f6fc4` snapshot under the gitignored
`target/perf-src/`, never the dirty tree. The solver hot-path files
(`src/vbd.rs`, `src/vbd/prediction.rs`, `src/ogc/*`, `src/solver.rs`,
`src/contact.rs`, `src/model.rs`, `src/energy.rs`, `src/gpu_vbd.rs`,
`src/gpu_vbd/cache.rs`) were verified byte-identical to `16f6fc4` at measurement
time. `HEAD` has since moved to `db46440`; round 2 re-pins. `db46440` was
verified once by a manual local gate at commit time (format pass, 0 desktop
errors, 28 warnings, 0 CPU failures, 13 `cloth-fit` bin tests passed), but it
has **no continuous signal**, and the current tree is red from later
uncommitted work (`docs/CODE-QUALITY.md` Q-16). A one-time human gate is not a
regression guard; that gap is why the CI change is worth making.

**Hardware.** Apple M1, 8 cores (4 performance + 4 efficiency), **8 GB RAM**,
macOS 27.0 (26A428), arm64. rustc/cargo 1.99.0.

## ⚠ Read this before quoting any wall-clock number

This host was **swap-thrashing** for the whole session: `sysctl vm.swapusage`
reported **11.3 GB used of 12.3 GB** (≈74 MB free pages), with load averages of
6–130. At the peak, `pgrep -f 'cargo|rustc'` was empty, so the pressure came
from processes outside this workspace and could not be removed from here.

Consequences, applied throughout:

1. **CPU time per step (user+sys) is the primary metric.** It is far less
   sensitive to swap and scheduling than wall time.
2. **Every wall-clock number is a contended upper bound**, labelled as such. It
   measures the host, not the solver.
3. A **clean re-run is outstanding** and is round 2's first action. The Lead
   granted a "no team builds" window, but swap did not recover, so even those
   numbers are marked.

## Reproduce

```sh
export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$PWD/.cargo-home"

# Pinned source snapshot (never build the dirty tree for a baseline)
mkdir -p target/perf-src && git archive 16f6fc4 | tar -x -C target/perf-src

# Builds (measured in the table below)
cd target/perf-src
CARGO_TARGET_DIR="$OLDPWD/target/perf-cold-desktop" \
  /usr/bin/time -l cargo build --release --locked -j 2 --bin cloth-fit --example motion_probe

# CPU matrix: 6 scenes x 90 steps, summarised after a 30-step warmup
cd "$OLDPWD"
BIN=target/perf-cold-desktop/release/cloth-fit OUT=output/perf/cpu STEPS=90 THREADS=2 \
  scripts/perf/run_cpu_matrix.sh
scripts/perf/summarize_metrics.py --warmup 30 output/perf/cpu/tshirt-30/metrics.jsonl

# Allocations (interposer, no source change)
clang -O2 -dynamiclib -o scripts/perf/libmalloc_count.dylib scripts/perf/malloc_count.c
DYLD_INSERT_LIBRARIES="$PWD/scripts/perf/libmalloc_count.dylib" \
  target/perf-cold-desktop/release/cloth-fit --garment tshirt --mesh-mm 30 --steps 90 --output output/perf/alloc-90

# GPU (opt-in, headless) and the one window smoke (both pre-approved by the Lead)
target/perf-cold-desktop/release/cloth-fit --garment tshirt --mesh-mm 30 --steps 120 --threads 2 --gpu-sim --output output/perf/gpu-tshirt-30
target/perf-cold-desktop/release/cloth-fit --garment tshirt --mesh-mm 30 --steps 0 --ui-perf-smoke --output output/perf/ui
```

`scripts/perf/run_cpu_matrix.sh` records wall, user, sys, peak RSS and the load
average for each run; `scripts/perf/summarize_metrics.py` prints nearest-rank
P50/P95 per metric field. Both are standard-library only. The repository's own
`scripts/*.ps1` benchmarks cannot run here: this machine has no `pwsh`.

## 1. CPU solver cost (default backend: CPU f64 VBD + OGC)

`cloth-fit`, `--threads 2`, `--iterations 16`, `dt = 1/60 s`, 90 steps, warmup
30, `16f6fc4`. `step_ms` is the in-process step timer; "CPU ms/step" is
`(user+sys)/90` from `/usr/bin/time -l`.

| Scene | Vertices | Triangles | step P50 | step P95 | detection P50 | solve P50 | CPU ms/step | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| tshirt 40 mm | 1,026 | 1,838 | 144.8 ms | 184.7 ms | 11.2 ms | 135.6 ms | 300.7 | 45.7 MB |
| tshirt 30 mm | 1,744 | 3,212 | 228.4 ms | 286.8 ms | 14.5 ms | 213.7 ms | 458.4 | 43.8 MB |
| tshirt 25 mm | 2,454 | 4,570 | 327.6 ms | 397.4 ms | 20.1 ms | 306.9 ms | 625.6 | 45.5 MB |
| dress 40 mm | 1,656 | 3,070 | 167.2 ms | 248.7 ms | 5.9 ms | 162.1 ms | 335.3 | 40.7 MB |
| dress 30 mm | 2,808 | 5,324 | 279.2 ms | 312.2 ms | 8.8 ms | 268.0 ms | 564.6 | 46.0 MB |
| dress 25 mm | 4,094 | 7,800 | 382.3 ms | 428.8 ms | 12.9 ms | 368.3 ms | 786.1 | 57.5 MB |

All wall figures are contended upper bounds; treat the CPU ms/step column as the
load-robust comparison.

**The solve dominates.** Detection is **3.2–7.7%** of the step across the six
scenes (lowest: dress 30 mm; highest: tshirt 40 mm), e.g. 14.5 ms of 228.4 ms at
tshirt 30 mm. `solve_ms = step_ms - detection_ms` and contains the 16 VBD
iterations plus warm start and state commit. **Making detection free would leave
the CPU path ~13× short of real time.** Step cost scales roughly linearly with
vertex count (1,026 → 2,454 vertices gives 144.8 → 327.6 ms).

### Detection breakdown P50 (OGC `contact_profile`)

| Scene | refit | candidates | narrow | candidate pairs | active contacts | retained terms |
|---|---:|---:|---:|---:|---:|---:|
| tshirt 40 mm | 0.140 ms | 3.84 ms | 6.35 ms | 224,942 | 148 | 5,111 |
| tshirt 30 mm | 0.171 ms | 5.86 ms | 7.82 ms | 282,628 | 212 | 7,542 |
| tshirt 25 mm | 0.220 ms | 8.59 ms | 10.88 ms | 430,378 | 309 | 9,748 |
| dress 40 mm | 0.094 ms | 3.69 ms | 1.87 ms | 56,805 | 70 | 2,777 |
| dress 30 mm | 0.137 ms | 5.66 ms | 2.67 ms | 108,665 | 98 | 3,719 |
| dress 25 mm | 0.182 ms | 8.58 ms | 4.22 ms | 185,005 | 139 | 4,412 |

Observation: the candidate cache rebuilds ~0.95–1.13 times per step and retains
**~37× more candidate pairs than retained contact terms** on tshirt 30 mm
(282,628 vs 7,542). The conservative envelope is doing what it promises
(no missed contacts) but it is the obvious lever *if* detection ever becomes the
wall. At 6% of step it is not, today, on CPU.

### Solve scaling

`tshirt` 30 mm, 60 steps, warmup 10, P50:

| Iterations | step P50 | solve P50 | detection P50 |
|---|---:|---:|---:|
| 8 | 143.9 ms | 112.3 ms | 24.0 ms |
| 16 (default) | 255.4 ms | 221.5 ms | 23.2 ms |
| 24 | 376.8 ms | 344.5 ms | 23.5 ms |

Solve cost is linear in iterations (≈13.9 ms per iteration). Halving iterations
nearly halves the step; the quality cost of that trade is documented in
`docs/adaptive-initialization.md` and is a product decision, not a free win.

| Threads | step P50 | solve P50 | detection P50 |
|---|---:|---:|---:|
| 1 | 469.6 ms | 410.0 ms | 35.1 ms |
| 2 (default) | 249.1 ms | 209.1 ms | 22.6 ms |
| 4 | 158.2 ms | 129.7 ms | 22.9 ms |

Scaling is sublinear (1.9× at 2 threads, 3.0× at 4) — expected on a 4P+4E M1
where the parallel work is spread over performance cores. Detection barely
scales, consistent with its small share.

### Run-to-run noise floor (same binary, same settings)

The whole matrix was run **twice** on the same pinned binary: run 1 at swap
11.3 GB / load 8–14, run 2 at swap 9.7 GB / load 4–18. Same scenes, 90 steps,
warmup 30, threads 2.

| Scene | CPU ms/step r1 → r2 | Δ | step P50 r1 → r2 | Δ |
|---|---:|---:|---:|---:|
| tshirt 40 mm | 300.7 → 291.4 | −3.1% | 144.8 → 139.6 | −3.6% |
| tshirt 30 mm | 458.4 → 459.1 | +0.1% | 228.4 → 230.1 | +0.7% |
| tshirt 25 mm | 625.6 → 652.8 | +4.4% | 327.6 → 350.1 | +6.9% |
| dress 40 mm | 335.3 → 321.8 | −4.0% | 167.2 → 161.8 | −3.2% |
| dress 30 mm | 564.6 → 625.7 | **+10.8%** | 279.2 → 335.8 | **+20.3%** |
| dress 25 mm | 786.1 → 827.3 | +5.2% | 382.3 → 402.6 | +5.3% |

**Noise floor: mean |Δ| 4.6%, max 10.8% on CPU ms/step; mean |Δ| 6.7%, max 20.3%
on a single run's step P50.** Peak RSS moved only 40.7–58.3 MB across both runs.
Consequence for the standing duty — **no round-2 claim below ~11% on CPU ms/step,
or ~20% on a single-run step P50, is a result.** Re-run the scene or report it as
within noise. The worst outlier (dress 30 mm) is also the row with the highest
load in both runs, so the variance is contention, not the solver.

### Reference Newton engine (`--reference`, same scene, 30 steps)

The VBD path exposes only detection vs solve. The legacy Newton reference
exposes a real breakdown, requested by the charter:

| Field | P50 | P95 |
|---|---:|---:|
| `step_ms` | 165.2 ms | 283.7 ms |
| `assembly_ms` | 122.4 ms | 184.9 ms |
| `line_search_ms` | 13.4 ms | 40.0 ms |
| `line_search_backtracks` | 0 | 0 |

Assembly is ~74% of the reference step; line search ~8%. This is the *oracle*
engine, not the shipped default, and it is comparable in cost to CPU VBD at the
same mesh here.

## 2. Allocations per step

Measured by interposing `malloc`/`calloc`/`realloc` from a `DYLD`-inserted
library (`scripts/perf/malloc_count.c`) on the unmodified binary. `free` is not
interposed.

| Steps | malloc | calloc | realloc | Requested bytes |
|---:|---:|---:|---:|---:|
| 0 (setup) | 178,056 | 395 | 6,076 | 68.7 MB |
| 30 | 498,946 | 477 | 795,711 | 1,600 MB |
| 90 | 724,713 | 633 | 1,292,319 | 2,521 MB |

**Per step, over the 0 → 90 difference: ~20,400 allocation calls, of which ~70%
are `realloc`, requesting ~27 MB.** This is whole-process and includes the
per-step metrics serialisation, so it is an upper bound on solver-only
allocation; the count is measured, the attribution below is from source.

Dominant sites, by reading the step path (no profiler available — see limits):

| Site | Why it allocates every step |
|---|---|
| `src/vbd.rs:189-226` `colors()` | `vec![Vec::new(); n]` plus one `push` per candidate-pair vertex into the inner vectors; `vec![usize::MAX; n]` twice; growth per colour. Contact terms are ~5,000–9,700 per step here. |
| `src/ogc/world.rs:362-375` `detect()` | `vec![query_radius; n]`, `Vec::new()` for `found` grown by every candidate, `incident = vec![Vec::new(); moving]` plus per-term pushes, `centers`/`radii` `to_vec`/`collect`. |
| `src/vbd.rs:385-395` | Per iteration **and per colour**, `let updates: Vec<_> = color.par_iter()...collect()`. At 16 iterations this dominates the *call count* even though each vector is small. |
| `src/vbd.rs:316-327`, `src/vbd/prediction.rs:15-41,43-52` | `state.v.clone()`, `guess()` collect, `target`/`inertia` collects, and `record()`'s `acceleration` collect plus full `state.clone()`. |

The allocation *count* is large, but at ~50 ns per call it is on the order of
1 ms/step against a 228 ms step — roughly **0.5%**. This matters as hygiene and
as a low-risk first change, not as a headline win.

## 3. Memory

| Process | Peak RSS |
|---|---:|
| `cloth-fit` CPU, tshirt 30 mm (smallest) | 43.8 MB |
| `cloth-fit` CPU, dress 25 mm (largest) | 57.5 MB |
| `cloth-fit` hybrid GPU, tshirt 30 mm | 74.2 MB |
| `cloth-fit` atelier window (`--ui-perf-smoke`) | 199.4 MB |

The application itself fits comfortably in 8 GB. The *development host* did not:
the toolchain plus its build artifacts drove the machine into 11.3 GB of swap.

## 4. GPU (hybrid path, Apple M1 / Metal)

One pre-approved headless run: `--gpu-sim`, tshirt 30 mm, 120 steps, threads 2.
Adapter reported: `Apple M1 (Metal)`. Hardware timestamps are available.

| Field | P50 | P95 |
|---|---:|---:|
| `step_ms` | 20.89 ms | 42.93 ms |
| `detection_ms` (CPU) | 7.54 ms | 13.63 ms |
| `solve_ms` (= step − detection) | 13.17 ms | 30.48 ms |
| `gpu_compute_ms` (hardware timestamp) | 11.37 ms | 28.00 ms |
| candidate pairs | 84,271 | 183,638 |

Mean over 120 steps: **23.30 ms/step**, i.e. ~10× faster than the CPU path
(228.4 ms P50) on the same scene and settings.

**Is it GPU-bound? No — not purely.** GPU compute is **54%** of the step
(11.37 / 20.89); CPU detection is **36%** (7.54); the remaining **8.6%**
(1.80 ms) is host packing, uploads and readback. Both halves matter.

**Transfers per step**, from `src/gpu_vbd/cache.rs:52-127` and
`src/gpu_vbd.rs:260-430` (read from source, not runtime-traced — there is no GPU
transfer counter):

- 8 `queue.write_buffer` calls per step, every step, with no content diffing:
  positions (`n*16 B` = 27,904), targets (`n*16`), ranges (`n*16`), balls
  (`n*32` = 55,808), order (`n*4` = 6,976), plus `terms`, `incident` and the
  uniform config block (alignment × colours). The five `n`-scaled buffers alone
  are **146,496 B/step**; `terms`/`incident` are not instrumented and stay
  unknown. The intrinsic mesh terms are re-uploaded every step even though only
  the contact/grab tail changes.
- Readback: `copy_buffer_to_buffer` of `n*16` = **27,904 B** positions plus 16 B
  of timestamps, followed by a **blocking**
  `device.poll(PollType::wait_indefinitely())` and `rx.recv()` — the host waits
  for the GPU every step. On unified memory this is a copy plus a sync, not a
  PCIe transfer, but it still serialises the two.

Because the entire `solve_ms − gpu_compute_ms` budget is only 1.80 ms (8.6% of
the step), removing all per-step transfer cost saves **at most ~9%**. The larger
GPU-side lever is the CPU detection inside the hybrid step (36%).

**Trajectory parity — measure, do not assume.** The hybrid path is *slower to
converge per step*, not divergent:

| Run | Steps (simulated) | Max seam gap at end |
|---|---:|---:|
| CPU VBD | 90 (1.5 s) | 1.59 mm |
| GPU hybrid | 120 (2.0 s) | **84.14 mm** |
| GPU hybrid | 360 (6.0 s) | **1.83 mm** |

At equal simulated time the GPU path is far behind; by 6 s it reaches the CPU's
seam closure. I did **not** isolate the cause (f32 vs f64, adaptive-initialisation
validity, contact handling) — tagged Unknown, and not something this round
should guess at.

## 5. Build and startup

All builds `--locked -j 2` from the pinned snapshot. Wall figures were taken
under load 8–55 and the host was swapping: **upper bounds**.

| Build | Wall | user | sys | Peak RSS |
|---|---:|---:|---:|---:|
| Cold `--features desktop` (cloth-fit + motion_probe) | **238.1 s** | 351.0 s | 25.5 s | 670 MB |
| Cold `--no-default-features` (cloth-fit + marvelous-cloth + motion_probe) | **53.5 s** | 95.1 s | 5.9 s | 686 MB |
| Incremental, no-op | **0.42 s** | 0.08 s | 0.12 s | — |
| Incremental, crate-only (`cargo clean -p marvelous-cloth`) | **21.6 s** | 34.8 s | 1.7 s | — |

| Binary | `--features desktop` | `--no-default-features` |
|---|---:|---:|
| `cloth-fit` | 20,219,984 B (19.3 MB) | 2,465,552 B (2.35 MB) |
| `motion_probe` | 7,413,200 B | 2,174,560 B |
| `marvelous-cloth` | (default-run) | 2,553,168 B |

The desktop feature multiplies the CLI binary 8.2× because it links eframe/wgpu.
`--no-default-features` is the fast build and the CPU test suite's build.

**Time to first frame (Derived).** `--ui-perf-smoke` process wall was 7.38 s
total; the perf module measures a 5.01 s window after a 1 s warmup, so
scene compile + window creation + first frame ≈ **1.4 s**. Peak RSS 199.4 MB.
This is a subtraction from two instrumented numbers, not a traced first-frame
time.

**Window cadence on M1 (hybrid GPU atelier).** The smoke **failed its own gate**,
which is the expected result here, and the numbers are useful anyway:

| Field | Value |
|---|---:|
| UI frames/s | **60.02** (gate ≥ 55) |
| Physics steps/s | 44.47 |
| Simulation speed | **0.741×** (gate ≥ 0.95) |
| UI frame interval P95 | 25.29 ms |

So the atelier holds 60 fps on an M1 while the physics delivers 0.74× real time.
For scale: the hybrid step is 20.89 ms P50 → 47.8 steps/s ideal, matching the
44.47 measured. The published 60 frames/s *and* ~60 steps/s figures are RTX 3090;
**they were not reproduced here.**

## 6. Low-end proxy (~1/4 of this machine's CPU and GPU budget)

This is the user's actual constraint, so the consequence is stated in steps per
second and distance from real time, not in milliseconds.

| Path | Now (M1) | ×4 slower CPU+GPU | Real-time target |
|---|---:|---:|---:|
| CPU VBD (1,744 verts) | 228 ms/step → 4.4 steps/s (0.073×) | ~920 ms/step → **1.1 steps/s (0.018×)** | 16.7 ms/step |
| Hybrid GPU (same scene) | 20.9 ms/step → 47.8 steps/s (0.80×) | ~78 ms/step → **12.8 steps/s (0.21×)** | 16.7 ms/step |

Derivation of the ×4 hybrid estimate: GPU compute 11.37 → 45.5 ms, CPU detection
7.54 → 30.2 ms (if the CPU is also 4× slower), plus ~2 ms host overhead.

**What dominates there:** on the CPU path the solve is already the wall, and it
is ~13.6× too slow on this M1 *before* the low-end discount; a 4× slower CPU
makes it ~55× too slow. Detection optimisation cannot fix that. On the hybrid
path neither half wins alone: GPU compute ≈ 58% and CPU detection ≈ 39% of the
projected low-end step, and on the integrated GPUs typical of low-end hardware
both share one memory bandwidth pool, so "GPU vs CPU" matters less than the
bandwidth both contend for. The honest summary is: **the default CPU path is not
usable interactively on low-end hardware, and the hybrid path is roughly 5× short
of real time there.**

## 7. What I could not measure, and why

- **No function-level profile.** `xctrace` requires a full Xcode install; this
  host has only Command Line Tools. The split *inside* `local()` (membrane vs
  bending vs contact vs seams) is Unknown, so I cannot rank micro-optimisations
  inside the solve by measured share.
- **No GPU transfer trace.** Transfer counts and sizes are read from source.
  `terms`/`incident` byte counts are not instrumented.
- **Allocation attribution is by source reading**, not by a heap profiler; the
  count is measured, the per-site split is Derived.
- **The apron does not exist in this repository.** `GarmentKind` is
  `Tshirt | Dress` (`src/garment/presets.rs:15-18`); the apron scenes in the
  validation docs are private. I measured tshirt and dress as an explicit
  substitution. A roadmap milestone that names apron scenes cannot be verified
  by anyone outside the author's machine.
- **Historical figures were not reproduced.** `docs/turbo-perf.md`,
  `docs/jitter.md` and `docs/performance-research.md` report 4–6 ms/step and
  ~60 steps/s **for the hybrid GPU backend on an RTX 3090**. That is a different
  backend on different hardware; this document does not quote it as comparable,
  and the repository had **no CPU-only step-time baseline** before this file.
- **Wall clock is host-contaminated** by the swap state above.
- `scripts/*.ps1` (including `bench-cpu.ps1`, which benchmarks the *legacy
  Newton* `marvelous-cloth --headless`, not the shipped default) cannot run: no
  `pwsh`.

## 8. Ranked optimisation backlog

Ordered by expected effect on the **default shipped path**, with risk. No item
here is approved for round 1; this is the queue for an optimisation window.

| # | Change | Expected impact | Risk | Evidence |
|---|---|---|---|---|
| 1 | Reduce solve cost per iteration (memory layout / avoiding duplicate `Jet` evaluation in `local()`), or accept fewer iterations with a measured quality gate | Largest: solve is 94–97% of the CPU step; iterations are linear at ~13.9 ms each | High: physics quality. Any change needs the energy/AD tests and a same-scene quality comparison | §1 |
| 2 | Move OGC detection (broad+narrow) to the GPU / keep candidates resident | Up to 36% of the *hybrid* step | High: correctness of the f64 CPU oracle; "no contact ignored" invariant | §4 |
| 3 | Stop re-uploading static intrinsic terms and diff the changed tail; avoid the blocking readback each step | ≤9% of the hybrid step (solve − gpu_compute = 1.80 ms) | Medium: buffer invalidation bugs | §4 |
| 4 | Reserve/scratch-buffer the per-step vectors (`colors`, `detect`, `updates`) | ~0.5% by call-count estimate — hygiene, not a headline | Low: keep allocation *order* and summation order identical | §2 |
| 5 | Tighten the conservative candidate envelope (282k pairs → 7.5k terms, 37×) | Only matters if detection becomes the wall; near-zero today on CPU | Medium: the envelope is a coverage contract, not a heuristic | §1 |

The quality pair's CI item needs its ordering attached, or it turns CI red on
the first push: the commit gate reports **28 desktop warnings**, and the
advisory job's existing `clippy --features desktop --all-targets -- -D warnings`
would fail on them. So the order is: clear the warnings, then flip the job to
gating, then add a timing/allocation assertion somewhere it can run (see the
shared agenda below). None of that is a performance item; it is the
prerequisite for having one.

## 9. Quality + efficiency agenda (shared with tf-cleanup / task-11)

Status: **FINAL.** Everything below this line is identical in
`docs/CODE-QUALITY.md` and `docs/PERFORMANCE.md`; only the heading differs,
because `docs/PERFORMANCE.md` numbers its sections. Do not edit one copy without
re-syncing the other. `tf-cleanup` owns the structure and buildability half;
`tf-performance` (`register`) owns the measured half and every number in it.

**Quality — `tf-cleanup` / task-11**

1. **Make the desktop job gating, and add `cargo test --features desktop`.**
   The advisory job's exact commands exit 0, and
   `cargo test --features desktop` runs 106 tests and ignores the 3 GPU tests with
   no GPU attached — pass/ignore/fail is load-independent, which is what carries
   this recommendation; the 41.6 s it took is an **upper bound** on a
   swap-thrashing host, not a baseline. At the measurement pin `16f6fc4`,
   **6 232 of 14 539 `src/` lines (42.9 %)** were compiled only under `desktop`
   with **zero** tests executed in them; `db46440` has since grown `src/` to
   58 files / 16 756 lines and shipped ~2 500 desktop-only lines. Those were
   verified once by a **manual gate** (`desktop_errors=0`,
   `bin_tests=13 passed`) — but a manual check leaves **no continuous signal**,
   and the only job that could supply one is the `continue-on-error` job that
   cannot fail. This is the largest coverage gain per unit of CI cost, and the
   prerequisite for any performance guard.
2. **Retire or justify `tracing` + `tracing-subscriber`.** 15 of the 83 CPU
   packages (18 %) exist for one `tracing::debug!` (`src/solver.rs:718-724`), and
   the subscriber is installed only in `src/main.rs:47-51` — so the log is dead in
   `cloth-fit`, the documented primary entry point. `register`'s build matrix
   below sizes the prize.
3. **Perf-regression coverage is structurally impossible today.**
   `--no-default-features` = 106 tests; `--features desktop` = those 106 plus the
   7 `src/atelier/settings.rs` unit tests that `db46440` wired in, with the 3
   hardware tests still ignored. There is no timing, allocation or
   transfer-budget assertion anywhere in the suite, and item 1 is the prerequisite
   for adding one.
4. **Shared construction of a protected invariant.** The
   `Triangle { rest, inv_dm, area }` derivation is written twice
   (`src/model.rs:176-189`, `src/garment/polygon.rs:180-187`) although
   `docs/architecture.md:60` makes it a contract.
5. **[perf-sensitive] coordination rule both forces accept:** any change whose
   cost is not provably zero on a measured hot path waits for `register`'s
   before/after on the same scene. All four round-1 candidates were adjudicated
   and **none was touched**:
   - `src/vbd.rs:210-226` — hot path, but **check-only is free**; approved on the
     strict condition that the *colouring itself* is not modified (any reorder
     changes trajectories bitwise). See Q-13.
   - `src/gpu_vbd.rs:362,370` — opt-in diagnostic, **free, approved**. See Q-13.
   - `src/blocksolve/sym_mat3.rs:67` — dead code is link-eliminated, no runtime
     cost; **approved on performance grounds**; removal is an API-surface call.
     See Q-05.
   - `src/solver.rs:346-367` vs `640-661` — **not on the default hot path**
     (reference solver only), so a quality win rather than a performance win, and
     **medium** correctness risk because that energy is the finite-difference
     oracle. See Q-09.
6. **The recorded CPU baseline measures the wrong engine.**
   `scripts/bench-cpu.ps1:19` drives `marvelous-cloth --headless` (legacy
   Newton), while the shipped CLI default is CPU VBD+OGC and the atelier's default
   is the hybrid GPU backend. Both forces agree this is a measurement gap; neither
   has changed it. See Q-33.

**Efficiency — `tf-performance` / task-12 (verbatim, `register`'s own text)**

1. **The shipped default is dominated by the VBD solve, not detection.** CPU
   VBD + OGC is 228.4 ms/step P50 at 1,744 vertices, of which detection is
   14.5 ms (6%). Any efficiency work aimed at real-time behaviour on low-end
   hardware has to move the solve, or move the solve off the CPU — BVH tuning
   alone cannot get there.
2. **CPU time per step is the only defensible metric on a swapping host.**
   Wall clock here measures the machine: the same scene ran 31 s and then
   112 s for 90–120 steps at different swap states. Publish `(user+sys)/step`
   with the load and swap state beside it.
3. **Per-step allocation is ~20,400 calls (~70% `realloc`, ~27 MB requested),
   and it is concentrated in `colors()`, `detect()` and the per-iteration
   `updates` collect.** This is hygiene and a low-risk first change, not the
   bottleneck; do not let an allocation-count improvement be reported as a
   speed-up without a wall/CPU before-after.
4. **The hybrid GPU path is faster but not GPU-bound**: GPU compute is 54% of
   the step, CPU detection 36%, and the host blocks on a readback every step.
   It also converges more slowly per step than the CPU oracle at equal settings
   (84 mm vs 1.6 mm seam gap at 2 s; 1.8 mm by 6 s) — a parity question to
   settle before trusting it on weak hardware.
5. **Every published performance figure in this repository is RTX 3090 +
   hybrid GPU and none was reproduced on this M1.** A number that cannot be
   reproduced on the machine the user owns is not a baseline; the tables in
   `docs/PERFORMANCE.md` are.
6. **Do not size CI budgets from any timing in this round.** The host was
   swapping for the entire session; the build numbers are upper bounds.

**Where the measured numbers live.** `register`'s build matrix is
`docs/PERFORMANCE.md` §5 and is deliberately **not** duplicated here; the same
file carries per-step CPU cost (§1), allocations (§2), memory (§3), the hybrid
GPU path (§4), the low-end proxy (§6) and what it could not measure (§7). Every
wall-clock figure in this repository is an upper bound: the host held 11.3 GB of
12.3 GB swap for the whole session with load 44–130 and no cargo running. Its
contention-corrected cold desktop figure is **238.1 s** (load 55 decaying to 17),
slightly worse than the earlier 216 s, and it asks that neither be presented as
clean. The useful, load-robust comparison is the ratio: **`--no-default-features`
builds cold in 53.5 s vs 238.1 s for `--features desktop` — 4.4× — and the
desktop CLI binary is 8.2× larger** (20 219 984 B vs 2 465 552 B).
