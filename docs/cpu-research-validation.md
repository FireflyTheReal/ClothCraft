# CPU contact / authoring validation — 15 October 2026

No GPU test, native window, shader dispatch or GPU benchmark was run. The owner was rendering in Unreal. All compilations and test commands use two jobs/test threads; contact replays use two Rayon workers. These results do **not** validate the currently unavailable custom montage's jitter.

## Measured contact work

Two previous local fitting states, same geometry and settings for both executables. Baseline contact/solver code is from commit `20b4db9064e5195f1843a9d75708fb77b6989eba`: the new replay harness was compiled before the contact/coloring edits. Authoring changes were already in the working tree but are not exercised by the harness. Ryzen 7 2700X, Rust 1.97.1 release binaries, sequence baseline/candidate/candidate/baseline, 40 queries per run, first five discarded for timings. No compilation ran concurrently with these replays. External Unreal workload means wall times remain indicative. Input SHA-256 hashes and per-run medians are recorded in the JSON summary.

Each query applies a deterministic 0.2 mm sinusoidal perturbation around the saved configuration. It is **not a physical trajectory**. The two scenes have active contacts; these are not empty flat-piece timings. The timed operation is `OgcWorld::detect`, including refit/candidate work/narrow phase and contact construction. It excludes coloring, stepping, GPU transfers and rendering. Constructor and validation costs are outside the timer. The timed steady-state samples all reused the candidate cache; first queries and cache rebuild correctness are covered by equivalence tests, not represented by this median.

| Local fixture | Vertices | Median detection before → after | Change | Exact VF/EE queries avoided |
|---|---:|---:|---:|---:|
| Saved T-shirt | 2,454 | 4.680 → 4.322 ms | −7.7% | 60.9% |
| Saved apron | 3,139 | 4.895 → 3.867 ms | −21.0% | 62.1% |

The reduction in exact evaluations is deterministic; the reduction in wall time depends on other work and hardware. Remaining exact pairs, contact construction and memory traffic still cost time. Do not equate 61% fewer queries with 61% less full-step time. Incidence-based coloring is verified for identical ordering but has no separate timing claim in this report.

All 40 per-query fingerprints match across all four runs on each scene, including warmup: canonical contacts, mollifier parameters and bit patterns of every displacement radius. CPU unit tests additionally compare counts, incidence, distances and radii directly. Fingerprints are regression evidence, not a formal floating-point proof or a physical-quality metric.

[Machine-readable summary](cpu-research-results.json). Local raw runs are under `output/cpu-research/`; inputs were `output/workspace-polish/compare-baseline/fitting-project.json` and `output/apron-ready/fitting-project.json`. These historical local states are not bundled fixtures or the user's current unsaved montage. Reproducing the exact numbers requires those inputs and comparable machine load.

## Reproduction on a saved project

Build the same `examples/contact_replay.rs` harness against the baseline and candidate, retain both executables, and run each twice in alternating order. The example was initially compiled as `contact_probe`; it was renamed to `contact_replay` to preserve the existing derivative-probe example and its CLI. Both retained executables contain the same replay source.

```powershell
cargo build --release --locked --no-default-features -j 2 --example contact_replay
./target/release/examples/contact_replay.exe --project output/my-session/fitting-project.json --output output/replay-candidate-1.json --repeats 40
python scripts/analyze-contact-replay.py --baseline output/replay-baseline-1.json output/replay-baseline-2.json --candidate output/replay-candidate-1.json output/replay-candidate-2.json
```

The analyzer rejects changed contact/radius fingerprints. Keep project bytes, settings and executable revisions with any shared report. A different project is a different experiment. Use the full [solver challenge](solver-challenge.md) before claiming faster physical simulation.

## Correctness checks

Completed commands:

```powershell
./scripts/check-cpu.ps1
cargo clippy --locked --features desktop --all-targets -j 2 -- -D warnings
# RAYON_NUM_THREADS=2 for test execution
cargo test --locked --features desktop -j 2 --lib --bin cloth-fit --test pattern_clipboard -- --test-threads=2
```

All pass. The CPU script runs **126 tests**, formatting, warning-free CPU Clippy and five template validations. The desktop-enabled headless selection runs **17 library tests, 37 atelier tests and 6 clipboard tests** (library/clipboard coverage overlaps the CPU suite). Warning-free desktop Clippy checks all targets. The analyzer also rejects an intentionally corrupted contact/radius fingerprint. Local logs: `output/cpu-research-check-final.log`, `output/radial-desktop-clippy-final.log`, `output/radial-desktop-tests-final.log`.

- Current-box pruning versus the unpruned implementation: changing radii, reused/rebuilt candidate cache, sewn exclusions, large translations, active and dormant contacts; exact comparison of contact keys, incidence, radii and minimum distances.
- Incidence coloring versus an independently expanded conflict graph: identical color groups and vertex order, with fixed avatar vertices, contacts, empty graphs and mouse grabbing; no shared term inside a color.
- Clipboard: six transaction tests covering group seams, omitted external seams, fresh IDs, cross-document material collisions, geometry/metadata round trips, compatible physical state, mass, undo/redo and atomic failure. Group layout cannot change physical rest/state.
- Headless UI: real egui press/release selects a radial sector; cancellation and click-through blocking, window-edge placement and geometric rectangle selection tested without a renderer.

Native radial appearance, OS clipboard integration and the complete mouse workflow still need an explicitly authorized desktop acceptance pass. A compile and headless input test do not establish that visual acceptance. The existing installation was not relaunched or replaced.
