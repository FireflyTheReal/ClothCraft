# Code quality and buildability audit

Standing task force `tf-cleanup` (task-11). Owner: this file. Standing partner:
`register` (task-12, measurement). Round 1, 15 October 2026.

This is an honest audit, not a changelog. Several findings are **positive** — the
boundary invariant and the test suite are in better shape than the file sizes
suggest — and they are recorded so the next contributor does not "fix" them.

## How to read this document

- **Observed** — measured or read directly from the repository at the pinned
  revision. The command is given.
- **Inferred** — a conclusion drawn from observations; it could be wrong.
- Every finding carries a **risk** (consequence of changing it, not likelihood)
  and a concrete **proposed change**.
- Round 1 applies **nothing** in `src/`; see [Round 1 status](#round-1-status).

### Revision pin

All `file:line` citations and line counts are pinned to **HEAD `16f6fc4`**
("Make the interface plain English, widen the CLO3D register, ship DRAW-01")
unless a finding says otherwise.

The working tree moved substantially during this audit, and kept moving. `register`
and I therefore pin to the revision and re-pin explicitly when it changes:

| Path | State when first observed | State at time of writing |
|---|---|---|
| `src/atelier/settings.rs` | untracked, 348 lines, **not wired** (see Q-31) | unchanged |
| `src/atelier/icons.rs` | untracked, 848 lines (task-2) | unchanged |
| `src/atelier/workspace.rs` | modified, +75/−13 (task-2) | still modified |
| `src/render.rs` | — | **modified, 672 → 1 242 lines** (task in flight) |
| `src/atelier.rs` | — | modified |
| `src/gpu_bench.rs` | — | modified |
| `scripts/perf/` | — | new (task-12, `register`) |

**Consequence, stated plainly:** line numbers in this document are valid at HEAD.
For `src/render.rs`, `src/atelier.rs`, `src/atelier/board.rs`,
`src/atelier/authoring_ui.rs` and `src/atelier/workspace.rs` the working tree has
already diverged, so a citation there identifies the code at HEAD, not the code
you will find today. Re-resolve by symbol name, not by line number, until the
feature work freezes and this file is re-pinned.

Pinned size: **55 files, 14 539 lines** in `src/`, plus 3 120 lines in `tests/`
(3 084 in the 17 integration targets, 36 in the shared `tests/common/mod.rs`)
and approximately 927 lines in in-source `#[cfg(test)]` modules.

## Method and re-verification

Run from the repository root, with the documented environment:

```bash
export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$PWD/.cargo-home"
```

| Claim | Command | Result observed |
|---|---|---|
| CPU tests green | `cargo test --locked --no-default-features -j 2 -- --test-threads=2` | 106 passed, 0 failed |
| No dead private code | `cargo clippy --locked --no-default-features --all-targets -j 2 -- -D warnings` | exit 0, no warnings |
| Desktop code compiles and lints | `cargo clippy --locked --features desktop --all-targets -j 2 -- -D warnings` | exit 0, no warnings |
| Desktop tests are GPU-free | `cargo test --locked --features desktop -j 2 -- --test-threads=2` | 106 passed, **3 ignored**, 0 failed, 41.6 s |
| Dependency graph size | `cargo metadata --locked [--no-default-features] --format-version 1` | 83 packages CPU / 174 with `desktop` |
| Line counts | `git show HEAD:<path> \| wc -l` | see tables below |
| No broken doc links | a relative-link check over every `*.md` (ignoring code spans) | 0 broken in real prose |

---

## 1. Module boundaries

### Q-01 — The stated boundary holds. Observed. Risk: n/a (do not break)

`AGENTS.md:34` and `docs/architecture.md:3` require `model`, `energy`,
`garment`, `contact` and `solver` to stay independent of egui/wgpu. This is true
today, and more strictly than the contract asks.

```bash
grep -rnw -e egui -e wgpu -e eframe -e Pos2 -e Color32 -e Rect -e Stroke \
  -e Response src/model.rs src/energy.rs src/garment src/contact.rs \
  src/solver.rs src/math.rs src/blocksolve src/ogc src/vbd.rs src/vbd \
  src/fitting.rs src/avatar.rs src/authoring.rs src/project.rs src/fabrics.rs \
  src/picking.rs
```

One hit, and it is a doc comment (`src/blocksolve/mod.rs:24`). There is also
**no `f32`** anywhere in those modules — the CPU numerics are uniformly `f64`,
including the AD scalar (`src/math.rs:34-38`). No numeric module imports
`crate::render`, `crate::editor`, `crate::atelier` or `crate::gpu_vbd`.

The dependency direction is clean in the other direction too: `gpu_vbd.rs`
depends on the numerics (`src/gpu_vbd.rs:10`), not the reverse.

**Proposed change:** none. Keep this as the reference when adding a module.

### Q-02 — The boundary is enforced only by Cargo optionality. Inferred. Risk: Medium

The invariant holds as a *side effect* of `eframe`/`bytemuck`/`pollster`/`image`
being `optional` (`Cargo.toml:23-26`): a numeric module cannot name `egui`
because the crate is absent from the `--no-default-features` build. Nothing
asserts the property directly. If `eframe` ever becomes non-optional, or a UI
type is re-exported through an always-present dependency, the only guard
disappears silently — the build still succeeds.

**Proposed change:** one CI step in `.github/workflows/cpu.yml`, in the `cpu`
job, after the clippy step:

```yaml
- run: cargo tree --locked --no-default-features -e normal | grep -qE 'eframe|wgpu' && exit 1 || exit 0
```

or, more readably, a shell step that fails if either name appears. Risk: Low.

---

## 2. Concentration: god-files and god-functions

### Q-03 — Eight tracked files hold 41 % of `src/`. Observed. Risk: Medium

| Lines | File | Note |
|---:|---|---|
| 986 | `src/atelier/board.rs` | 2D interactions; a split is in flight (task-2) |
| 955 | `src/editor.rs` | legacy single-panel editor, desktop-only |
| 862 | `src/solver.rs` | Newton/PCG **plus** metrics, regularization, Armijo, diagnostics |
| 735 | `src/atelier.rs` | bin-included shell (see Q-26) |
| 672 | `src/render.rs` | desktop-only |
| 616 | `src/authoring.rs` | 332-line `apply` (see Q-04) |
| 602 | `src/atelier/authoring_ui.rs` | desktop-only |
| 547 | `src/gpu_vbd.rs` | desktop-only |

`src/solver.rs` is the most consequential. `docs/architecture.md:46` describes
it as "Implicit Euler, Newton/PCG, Armijo"; it also owns the 35-field `Metrics`
struct (`src/solver.rs:61-96`), geometry validation, a regularization loop
(`src/solver.rs:620-626`) and the end-of-step diagnostics block
(`src/solver.rs:640-690`).

**Proposed change:** in a refactor window, split `solver.rs` into
`solver/mod.rs` (API and `step`), `solver/metrics.rs` (`Metrics` plus the
reporting block) and keep the assembly/Newton core where it is. That is a
mechanical move, but it is a 862-line move — not round-1 work. Risk: Medium
(public paths such as `solver::Metrics` and `solver::SolverSettings` are used by
the bins and the atelier worker).

### Q-04 — Nine functions are 200+ lines. Observed. Risk: Medium

Approximate body extents (brace scan including nested closures, so treat as
±10 %):

| Lines | Location | Function |
|---:|---|---|
| 348 | `src/gpu_vbd.rs:125` | `GpuVbd::step` |
| 334 | `src/atelier.rs:401` | `Atelier::ui` |
| 332 | `src/authoring.rs:234` | `apply` |
| 287 | `src/render.rs:275` | `draw_scene` |
| 266 | `src/atelier/board.rs:720` | `paint_board` |
| 250 | `src/atelier/board.rs:470` | `board_events` |
| 225 | `src/solver.rs:502` | `step_problem` |
| 216 | `src/garment/template.rs:214` | `compile` |
| 202 | `src/atelier/authoring_ui.rs:296` | `authoring_properties` |

`authoring::apply` (`src/authoring.rs:234`) is the one I would split first: it
is a pure CPU command dispatcher with no UI dependency, so a split is testable
by the existing 11 tests in `tests/authoring.rs` and does not collide with the
desktop work.

**Proposed change:** extract per-variant handler functions from
`authoring::apply` behind a `match` that stays visible. Risk: Medium (612-line
file, but the function is a dispatcher and the tests are strong).

---

## 3. Dead code

### Q-05 — `SymMat3::add_scaled` is unused. Observed, not compiler-confirmed. Risk: Low

```bash
grep -rn 'add_scaled' --include='*.rs' .   # one hit: its own definition
```

`src/blocksolve/sym_mat3.rs:67-71`. A whole-repository text search over `src/`,
`tests/` and `examples/` finds no reference. It is `pub`, re-exported at
`src/blocksolve/mod.rs:33`, and therefore exempt from the `dead_code` lint —
`cargo` cannot confirm it is unused, which is exactly the round-1 bar.

**Proposed change:** delete it, or add a test that pins its behaviour if it is
intended as public API. Risk: Low (no in-repo caller; the crate has no external
consumers in this repository).

### Q-06 — There is no dead private code. Observed. Risk: n/a (positive)

- `cargo clippy --no-default-features --all-targets -- -D warnings` → exit 0.
- `cargo clippy --features desktop --all-targets -- -D warnings` → exit 0.
- `grep -rn '#\[allow(' src tests` returns exactly two hits, both deliberate and
  both narrow: `clippy::too_many_arguments` (`src/render.rs:246`) and
  `clippy::suspicious_arithmetic_impl` with a justifying comment
  (`src/math.rs:98-99`). There is no `#[allow(dead_code)]` anywhere.

A dead-code-unused-public scan of all 216 `pub` items in `src/` found exactly one
candidate (Q-05). Duplication and dead code are genuinely low here.

**Proposed change:** none. Keep `-D warnings` gating in CI.

### Q-07 — `tracing` and `tracing-subscriber` are the largest removable weight. Observed. Risk: Low

`cargo metadata` reachability over the CPU resolve graph:

- 83 packages in total;
- dropping `tracing` + `tracing-subscriber` removes **15** of them:
  `aho-corasick`, `lazy_static`, `matchers`, `nu-ansi-term`, `pin-project-lite`,
  `regex-automata`, `regex-syntax`, `sharded-slab`, `thread_local`, `tracing`,
  `tracing-attributes`, `tracing-core`, `tracing-log`, `tracing-subscriber`,
  `valuable` — **18 % of the CPU dependency graph**, pulled by
  `tracing-subscriber`'s `env-filter` feature (`Cargo.toml:21`).

What the tree does with all of it: exactly **one** call site,
`tracing::debug!` at `src/solver.rs:718-724`, and one subscriber installed at
`src/main.rs:47-51`. The subscriber exists **only** in the legacy
`marvelous-cloth` binary. `cloth-fit` — which `README.md:47` and
`docs/handoff.md:9` present as the primary entry point for the atelier — never
installs one, so that `debug!` is silently discarded there. Neither does
`cloth-template`.

**Proposed change:** `register` measures the cold-build and binary-size delta
(separate `CARGO_TARGET_DIR`s). If it is material, either drop both dependencies
and use an `eprintln!` gated by an env var, or keep them and install the
subscriber in all three binaries. Either way the current asymmetry is a
bug-shaped inconsistency. Risk: Low. **Do not act before `register` reports.**

---

## 4. Duplicated logic

Automated scan: an 8-line windowed clone detector over `src/` found **one**
cross-file duplicate window pair. Duplication is not a systemic problem here.

### Q-08 — The `Triangle`-from-rest-UVs derivation is written twice. Observed. Risk: Low

`src/model.rs:176-189` (rectangular FEM grid) and `src/garment/polygon.rs:180-187`
(constrained Delaunay) both do:

```rust
let rest = vertices.map(|v| mesh.uv[v]);
let dm = DMat2::from_cols(rest[1] - rest[0], rest[2] - rest[0]);
let area = 0.5 * dm.determinant().abs();
mesh.rest_area[v] += area / 3.0;   // model.rs only
mesh.triangles.push(Triangle { vertices, rest, inv_dm: dm.inverse(), area, material: 0 });
```

This is not incidental repetition: `docs/architecture.md:60` makes it a contract
("`Triangle.rest`, `inv_dm` and `area` derive only from the pattern").
Two copies of a stated invariant is one copy too many — the second copy is where
a future "small fix" will diverge.

**Proposed change:** one constructor, e.g.
`Triangle::from_rest(vertices, uv, material) -> (Triangle, f64 /* area */)`,
implementing the derivation once; both call sites use it. Risk: Low, but it
touches `model.rs` and `garment/polygon.rs` — a refactor window item.

### Q-09 — Membrane and bending sums computed twice per step. Observed, `[perf-sensitive]`. Risk: Low

`src/solver.rs:346-367` (`Problem::objective`, called once per Newton iteration
and once per line-search probe) and `src/solver.rs:640-661` (end-of-step
metrics) contain four near-identical `iter().map(...).sum()` blocks over
`mesh.triangles` and `mesh.hinges` with the same
`energy::membrane` / `energy::bending` calls.

**Proposed change:** two small free functions
(`membrane_energy(mesh, x, materials)` / `bending_energy(...)`) used by both.

**`register` adjudication (round 1):** *not on the default hot path.*
`src/solver.rs` is the reference Newton solver, reached only through
`cloth-fit --reference`; the shipped default is VBD+OGC (`src/vbd.rs`), so the
expected effect on the default is **zero**. On `--reference` the recompute is
once per step against many Newton iterations plus line-search probes, so it is
bounded well under one iteration. Verdict: **a quality win, not a performance
win — and it carries medium correctness risk.** The reference solver's energy is
the finite-difference oracle for the whole suite, so an extraction must preserve
the exact summation order or the AD tests change meaning.

Consequence for the backlog: this drops from item 10 to a window item, and the
proposed helper must be documented as order-preserving. Risk: **Medium**, not
Low — corrected upward from my initial rating.

### Q-10 — The validation pipeline exists twice and has already drifted. Observed. Risk: Low

| Step | `scripts/check-cpu.ps1` | `.github/workflows/cpu.yml` (`cpu` job) |
|---|---|---|
| `cargo fmt --all -- --check` | :9 | yes |
| `cargo test --no-default-features` | :10 | yes |
| `cargo clippy --no-default-features --all-targets -D warnings` | :13 | yes |
| 5 × `cloth-template validate` | :16-19 | yes |
| `cloth-template simulate --steps 30` | **absent** | yes |
| desktop compile/lint | **absent** | `desktop` job (non-gating) |

Two encodings of one pipeline, already different. A contributor who runs the
documented local check (`AGENTS.md:18`, `CONTRIBUTING.md:14`) does not run the
simulate step and never compiles desktop code.

**Proposed change:** see Q-24 (a single POSIX script both call) — this and Q-24
have one fix. Risk: Low.

---

## 5. Error handling

### Q-11 — Two conventions, with one library-level outlier. Observed. Risk: Low

| Shape | Count | Where |
|---|---:|---|
| `Result<_, String>` | 58 | library, consistently |
| `Err("…".into())` | 95 | library, consistently |
| `Box<dyn std::error::Error>` | 10 | 9 in bins/adapters, **1 in the library**: `src/garment/export.rs:132` |

`write_artifacts` (`src/garment/export.rs:127-132`) returns
`Result<(), Box<dyn std::error::Error>>`, and the function immediately below it
in the same file, `grain_guides_obj` (`src/garment/export.rs:149-153`), returns
`Result<String, String>`. That is the whole finding: one file, two conventions,
adjacent functions.

**Proposed change:** document the rule rather than churn code — "the library
returns `Result<_, String>`; `Box<dyn Error>` is reserved for process entry
points, where I/O and domain errors must mix." Concretely, add those two lines
to `docs/architecture.md` under *Contracts to preserve* (line 58). If the
outlier is changed, prefer leaving `write_artifacts` as `Box<dyn Error>` and
noting it as the one sanctioned exception; normalizing it would need `map_err`
on three I/O errors for no caller benefit. Risk: Low (doc-only preferred).

### Q-12 — `thiserror`/`anyhow` would not be earned. Observed. Risk: n/a (positive)

`grep -rn 'anyhow\|thiserror' src Cargo.toml` → 0 hits. Every error is a
human-readable `String`, and every consumer uses `?`, `is_err()`, or
`unwrap`/`expect` in a test. No caller branches on an error *cause*, which is
the only thing a typed error enum buys. Adding `thiserror` today would add
public API surface and a dependency for a consumer that does not exist —
directly against `AGENTS.md:38` ("avoid abstractions that anticipate an
implementation which does not exist").

**Proposed change:** none. Revisit only when a caller must distinguish causes
(e.g. a retry on a non-convergence error).

### Q-13 — Panics outside test modules: 47 sites, 2 worth attention. Observed. Risk: Low

Most are benign and should be left alone: `Mutex::lock().unwrap()` at
`src/atelier.rs:405` and `src/atelier/worker.rs:117` (no poisoning path), and the
`write!` family in `src/garment/export.rs:40-99` (writing into a `String` is
infallible — 16 of the 47 sites are this file).

Two deserve a note:

- `src/vbd.rs:210-226`, greedy vertex colouring. `seen` is `vec![usize::MAX; n]`
  and is indexed as `seen[color]`; `(0..).find(|c| seen[*c] != v)` is an
  *unbounded* search. Both are safe only while the colour count stays below `n`,
  which holds for any mesh that is not a complete graph. Nothing states or
  checks the assumption, so the failure mode is an out-of-bounds panic rather
  than an error. `[perf-sensitive]`
- `src/authoring.rs:136`, `serde_json::to_vec(&t).unwrap()` inside
  `same_physics` — the identity comparator on the edit path, not a test. The
  serializer fails on non-finite floats; template validation rejects those
  upstream, so the `unwrap` is currently unreachable, but it is a panic in a
  comparison function whose contract is "are these physically the same".

**Proposed change:** a bounded search plus a `debug_assert!(c < seen.len())` at
`src/vbd.rs:220`, and `?`-propagation or an explicit
`expect("template is validated before comparison")` with a comment at
`src/authoring.rs:136`. Both are small; both are inside code `register` may be
measuring, so Q-13 waits for a window.

**`register` adjudication (round 1):** on the colouring bound — *on the hot path,
but a check only.* Greedy colouring of `n` nodes needs at most `n` colours, so
`seen[c]` is in bounds in theory; the bound/`debug_assert` is free (no
allocation, no measurable change) and is **approved at zero measured cost**. One
rule attached: it must stay **check-only**. Any change to the *colouring itself*
reorders the per-iteration summation and changes trajectories bitwise — exactly
the kind of "cleanup" a performance force must adjudicate against. So the
backlog action is the assertion, not a rewrite of the search.

`src/gpu_vbd.rs:362,370` (Q-13's unnamed tolerances) was also adjudicated:
**free, no performance impact, approved** — it sits inside the opt-in
`check_gradients` diagnostic, not the hot path. Risk: Low.

---

## 6. Test structure

### Q-14 — The suite is strong, and property-shaped. Observed. Risk: n/a (positive)

106 CPU tests + 3 ignored GPU tests; 3 120 lines under `tests/` across 17
integration targets, plus ~927 lines of in-source test modules. These are not
constant-mirroring tests:

- `tests/fabrics.rs:36-48` recovers the membrane traction by central differences
  over the real energy rather than asserting the preset constant.
- `tests/numerics.rs` asserts finite-difference agreement between analytic and
  AD derivatives, objectivity, and balance.
- `tests/garment.rs:372-373` asserts `metrics.seam_j == seam.energy(x)` — the
  seam really is in the reported metrics, not just the objective.
- Adversarial cases exist: `corrupt_compiled_seam_returns_error_without_advancing_state`,
  `indefinite_operator_is_rejected_rather_than_returned`,
  `iteration_limit_is_not_reported_as_convergence`,
  `initial_intersection_is_rejected_instead_of_claiming_a_guarantee`.

`tests/` names read as claims, which is why the invariant review below could be
done at all.

### Q-15 — 42.9 % of `src/` is executed by no CI test. Observed. Risk: High (this is the top finding)

At HEAD, **6 232 of 14 539 `src/` lines across 17 files** are compiled only
under `feature = "desktop"`:

`src/atelier.rs` (735) + all of `src/atelier/` (2 342) + `src/render.rs` (672) +
`src/editor.rs` (955) + `src/gpu_vbd.rs` (547) + `src/gpu_vbd/cache.rs` (128) +
`src/gpu_bench.rs` (118).

The `cpu` CI job runs `--no-default-features`, so none of it is compiled *or*
tested there. The `desktop` job is `continue-on-error: true`, so failures cannot
fail the build. And the only desktop test target, `tests/gpu_motion.rs`, is
`#![cfg(feature = "desktop")]` with **all three tests `#[ignore]`**
(`tests/gpu_motion.rs:14,64,141`).

Result: **no CI job executes a single test in 43 % of the codebase.** The atelier
— 3 077 lines including the newest and fastest-moving files — has no
`#[cfg(test)]` module at all.

### Q-16 — The advisory desktop job is green here, and desktop tests are GPU-free. Observed. Risk: Low (make it gating)

I ran the advisory job's exact commands on this machine:

```
cargo clippy --locked --features desktop --all-targets -j 2 -- -D warnings   # exit 0
cargo test   --locked --features desktop -j 2 -- --test-threads=2
  → 106 passed; 3 ignored; 0 failed; 41.6 s
```

No GPU is touched: the three hardware tests are `#[ignore]`, so
`cargo test --features desktop` is safe for CI and does not violate
`AGENTS.md:22` ("No GPU test may be added to the suite that runs by default").

`src/atelier/settings.rs` — 348 lines with 7 unit tests, added by task-2 — is
proof that the team is already writing desktop unit tests that currently cannot
run anywhere.

**Proposed change (recommended to the Lead as the single highest-value change):**

1. Drop `continue-on-error: true` from the `desktop` job in
   `.github/workflows/cpu.yml`.
2. Add to that job: `cargo test --locked --features desktop -j 2 -- --test-threads=2`.
3. Update the three places that carry the stale caveat in the same pull request:
   `ROADMAP.md:17` (the P0.5 row), `ROADMAP.md:125` (the caveat bullet) and
   `docs/handoff.md:9` — "a **consultative** `desktop` job (`continue-on-error`,
   to be validated on Linux)". The last is `docs-english`'s file: coordinate.

Risk: Low. Cost measured at 41.6 s with warm dependencies. Note the job currently
uses `--bin cloth-fit --bin marvelous-cloth` for `check`; `--all-targets` on the
clippy step already covers tests and examples.

### Q-17 — No test asserts a budget. Observed. Risk: Medium

There is no timing, allocation, transfer-size or iteration-count *budget*
assertion anywhere in `tests/`. `ROADMAP.md:129` acknowledges the equivalent gap
for rendering ("The UI has no automated visual regression test"). This is
`register`'s lane; recorded here because it is a test-structure gap, and because
item Q-16 is its prerequisite: a perf guard has no home to live in today except
the CPU targets.

### Q-33 — The documented CPU baseline measures an engine the product no longer ships by default. Observed. Risk: Medium

Raised jointly with `register`, which found it from the measurement side; I
verified the mechanism in the script.

There are **three** execution engines, and the baseline script measures the
wrong one:

| Engine | Reached by | Covered by the 106 CPU tests? |
|---|---|---|
| Newton/PCG reference | `cloth-fit --reference`, `marvelous-cloth --headless` | yes |
| VBD + OGC, CPU `f64` | `cloth-fit` **default** (no `--gpu-sim`) | yes — `tests/vbd.rs`, `tests/ogc.rs`, `tests/ogc_bvh.rs`, `tests/ogc_cache.rs` all run under `--no-default-features` |
| VBD + OGC, hybrid GPU | the atelier window's worker (`src/gpu_vbd.rs`) | **no** — `src/gpu_vbd.rs` is desktop-gated, and its only test target has all three tests `#[ignore]` |

`scripts/bench-cpu.ps1:19` drives `target/release/marvelous-cloth --headless`,
i.e. the **legacy Newton** engine — while `docs/handoff.md:9` and
`AGENTS.md:5` describe the shipped default as VBD+OGC, and the atelier's default
as the hybrid GPU backend. So the script that produces the recorded CPU numbers
(`bench-cpu.ps1` feeding `output/cpu-suite/suite.json`) benchmarks neither the
CLI default nor the GUI default.

This is a sharper restatement of `register`'s point, which I am deliberately
weakening in one place: the CPU suite **does** exercise VBD+OGC on CPU, so
"CPU tests are green" is not silent about the shipped CLI path. What is silent is
the **GPU** engine the atelier actually ships, and what is misleading is which
engine the documented benchmark script measures.

**Proposed change:** none in round 1 (`register` owns the measurement; I own the
citation). In a window: either point `bench-cpu.ps1` at `cloth-fit` without
`--gpu-sim`, or rename it and state in `docs/performance-research.md` which
engine each recorded number came from. Risk: Medium — the risk is not the change,
it is that a reader currently takes those numbers as the product's baseline.

### Q-18 — Three `AGENTS.md` invariants have no direct test. Inferred. Risk: Medium

Reviewed against the invariant list at `AGENTS.md:26-34`:

| Invariant | Verdict |
|---|---|
| 2D metres are the rest state, never recomputed from the drape | **covered** — `extension_diagnostic_uses_material_rest_and_is_rigid_motion_invariant`, `free_fall_preserves_rest_shape_and_backward_euler_velocity`, `board_motion_is_not_a_physical_transform_and_does_not_stale_state_cache` |
| Shared material, per-panel grain, caches recompiled on change | **covered** — `material_binding_mass_grain_and_placement_are_independent`, `rotating_grain_90_swaps_warp_weft`, `material_edit_updates_mass_and_only_selected_binding_while_preserving_drape` |
| Edit files use panel/edge IDs and material coordinates, no persistent indices | **covered** — `deleting_and_moving_panels_maps_state_by_identity_and_local_topology`, `seams_survive_remaillage_and_panel_deletion_removes_references` |
| `ReferenceSolver::step`, `Project` v1, v1 templates preserved | **covered** — `project_roundtrip_and_invalid_inputs`, `numeric_dimensions_resize_a_legacy_rectangle_exactly`, `polygons_reject_crossings_reversed_winding_and_v1_schema` |
| Delaunay in the mesh metric, contour constraints, quality bounds, cache invalidation | **covered** — `cdt_refines_area_angles_and_preserves_all_named_contours`, `fitting_cache_rejects_remeshed_state_and_failed_step_is_transactional` |
| Seams in objective, gradient, Hessian **and metrics** | **partially covered** — gradient/Hessian by `seam_gradient_hessian_conservation_and_rigid_motion`, metrics by `tests/garment.rs:372-373`; but only for `ReferenceSolver`. `VbdMetrics` (`src/vbd.rs:61-82`) exposes **no energy field at all**, so the clause is unverifiable on the default VBD path (this converges with `register`'s lead at `src/vbd.rs:64-67`) |
| No arbitrary projection of positions after Newton | **not covered** — nothing fails if a projection is inserted; the closest guard is `contact_block_derivatives_and_force_balance` |
| Parameters dimensioned, penalty is not an intersection guarantee | **covered** — `initial_intersection_is_rejected_instead_of_claiming_a_guarantee`, `invalid_numeric_dimensions_leave_the_document_untouched` |
| Fiber guides add no yarns, contacts or calibration | **not covered** — `material_path_splits_at_fold_instead_of_cutting_through_surface` and `material_paths_recompile_after_remeshing_and_export_guides` cover geometry and recompilation, not the absence of a physical contribution |
| Numerics free of egui/wgpu | **covered indirectly** by `--no-default-features` compiling in CI (see Q-02 for the fragility) |

**Proposed change:** two cheap tests. (1) A VBD-side assertion that
`seam.energy(x)` is reflected in a reported number, matching the Newton
assertion — which first needs a field, hence the Q-18 field gap is really "add
`seam_j` to `VbdMetrics` or state that the VBD metrics intentionally omit
energies". (2) A guides test asserting that compiling guides changes no
`Mesh`/`Triangle`/`Hinge`/seam contribution to `objective`. Risk: Low, but (1)
touches `src/vbd.rs` and (2) needs an owner.

### Q-19 — Fixtures are duplicated across integration targets. Observed. Risk: Low

`tests/common/mod.rs` is 36 lines and offers one fixture
(`resting_patch`). `tests/vbd.rs:35` defines its own `fixture()`, and
`tests/fabrics.rs:17-25` builds an inline `Pattern`. Consolidating is
low-value churn; noted so it is not mistaken for a missing pattern. **Proposed
change:** none.

---

## 7. Build friction

### Q-20 — Feature coverage in CI. Observed. Risk: High

See Q-15 and Q-16. Summary: `--no-default-features` is the only enforced
configuration, so 43 % of `src/` has neither compile nor test enforcement.

### Q-21 — No declared MSRV. Observed. Risk: Low

`Cargo.toml` has no `rust-version` field. `edition = "2024"` (`Cargo.toml:4`)
requires Rust ≥ 1.85; CI uses a floating `dtolnay/rust-toolchain@stable`
(`.github/workflows/cpu.yml`). Consequences: a new stable can break the build
with no contract to point at, and an older local toolchain produces an edition
error instead of "this crate requires Rust 1.85".

**Proposed change:** add `rust-version = "1.85"` to `[package]` and one line to
`CONTRIBUTING.md`. Risk: Low (metadata only; verify with a `cargo check` — Cargo
warns, it does not fail, on too-new toolchains).

### Q-22 — 83 packages CPU-only, 174 with `desktop`. Observed. Risk: Medium

`cargo metadata` package counts in the resolve graph: **83** with
`--no-default-features`, **174** default. The 91-package difference is the cost
of the desktop feature, which is a default feature (`Cargo.toml:11`). Combined
with Q-07 (15 removable CPU packages for one log line) and Q-23, there is real
build-time headroom — but all of it needs `register`'s cold-build measurements,
not estimates.

### Q-23 — The dev profile is non-default in two ways, undocumented. Observed. Risk: Low

`Cargo.toml:31-32` sets `[profile.dev] opt-level = 1`; `Cargo.toml:34-35` sets
`opt-level = 2` for every dependency. So `cargo build` is slower to compile and
much faster to run than a stock dev profile, and a contributor comparing their
build time against a default project will be confused. `AGENTS.md` and
`CONTRIBUTING.md` do not mention it.

**Proposed change:** one sentence in `CONTRIBUTING.md` ("the dev profile is
deliberately `opt-level = 1` for the crate and `2` for dependencies; this trades
cold compile time for test and simulation speed"). Risk: Low.

### Q-24 — The documented check command is PowerShell-only. Observed. Risk: Low

`AGENTS.md:17-20` gives PowerShell as the "usual commands", and the complete
local check is `scripts/check-cpu.ps1`. There is no POSIX equivalent
(`scripts/` holds five `.ps1` files and two `.sh`/`.py`). Meanwhile the project
ships a macOS-only packaging script (`scripts/make-macos-prototype.sh`) and
produces `prototype/mac/ClothCraft.app`. A macOS or Linux contributor has no
single documented command that matches CI, and (Q-10) the PowerShell script has
already drifted from the workflow.

**Proposed change:** add `scripts/check-cpu.sh` implementing the identical step
list including the simulate step and the `--features desktop` check, reference
it from `AGENTS.md` and `CONTRIBUTING.md`, and let the CI `cpu` job invoke the
same script so there is one definition. Risk: Low (new file; note `AGENTS.md`
is the Lead's contract document — propose, do not edit unilaterally).

---

## 8. Naming and discoverability

### Q-25 — Four names for one project. Observed. Risk: Low

| Name | Where |
|---|---|
| `ClothCraft` (directory, README title, icon, bundle) | `README.md:1`, `docs/handoff.md` |
| `marvelous-cloth` (package) | `Cargo.toml:2` |
| `marvelous_cloth` (crate) | `src/main.rs:2` |
| `MARVELOUS CLOTH` (contract title) | `AGENTS.md:1` |

`README.md:7` explains the project is ClothCraft; nothing explains why the
package is called `marvelous-cloth`, or that `AGENTS.md` prefers the older name.

**Proposed change:** one sentence in `README.md` stating the convention and that
the package name is historical. Risk: Low. Coordinate with `docs-english`
(README is task-7's file).

### Q-26 — `src/atelier.rs` is not a library module, and its module name is `fitting_editor`. Observed. Risk: Low

```rust
// src/bin/cloth-fit.rs:9-11
#[cfg(feature = "desktop")]
#[path = "../atelier.rs"]
mod fitting_editor;
```

`src/gpu_bench.rs` is included the same way (`src/bin/cloth-fit.rs:12-14`), and
`src/editor.rs` is `mod editor` from `src/main.rs:14`. So:

- the file is `src/atelier.rs`, the module is `fitting_editor`, the docs call it
  "the atelier", and the only documentation that ever names the module is
  **wrong**: `docs/first-fit.md:57` refers to `fitting_editor.rs`, a file that
  does not exist;
- `src/atelier.rs` is invisible to `cargo doc` and to `--no-default-features`;
- `docs/architecture.md:56` lists `atelier.rs` in the responsibilities table
  without mentioning that it is bin-included rather than a library module.

Consequence for a newcomer: `grep -rn 'atelier' src/` and
`grep -rn 'fitting_editor' src/` return disjoint results, and neither matches
the prose.

**Proposed change:** state the three `#[path]` includes in the responsibilities
table in `docs/architecture.md:30-56`, one row each. Risk: Low (doc only).

### Q-27 — `docs/` has no currency index. Observed. Risk: Low

31 markdown files, including four explicitly historical
`*-earlier-pass.md` documents and several `*-validation.md` / `*-validation.json`
pairs. `docs/handoff.md` (75 dense lines) is the index, but its newest entry is
at the top and the reader must infer which documents are superseded.

**Proposed change:** add `Status: historical` banners to the four
`*-earlier-pass.md` files, and a 5-line table at the top of `docs/handoff.md`
mapping topic → current document. Risk: Low (doc only). `docs-english` owns
`docs/handoff.md` this round.

### Q-28 — Three broken relative links (**fixed in round 1**) and one path naming a file that never existed. Observed. Risk: Low

Three broken relative links, all in `docs/clo3d-feature-catalog-earlier-pass.md`:

| Line | Link | Was | Fixed to |
|---:|---|---|---|
| 28 | `[ROADMAP.md](ROADMAP.md)` | missing | `../ROADMAP.md` |
| 329 | `[FORMAT-NOTES.md](FORMAT-NOTES.md)` | missing | `clo3d-format-notes-earlier-pass.md` |
| 337 | `[ROADMAP.md](ROADMAP.md)` | missing | `../ROADMAP.md` |

**Status: the three links are FIXED (round 1).** `register` confirmed the file is
not its scope — task-4 owned only `docs/clo3d-capability-register.md` — so I
extended this task's declared write scope to include the file and applied the
change. Post-fix check: **0 broken relative links in real prose** across
`README.md`, `AGENTS.md`, `AI_START_HERE.md`, `CONTRIBUTING.md`, `ROADMAP.md`,
`THIRD_PARTY_NOTICES.md`, all of `docs/**` and all of `assets/**`/`data/**`
(the checker ignores code spans, so the deliberately-broken examples in this
table are not counted).

One item remains **not** mine to fix: `docs/first-fit.md:57` cites
`` `fitting_editor.rs` `` as "interface" — that file has never existed; the
module lives at `src/atelier.rs` and is included as `fitting_editor`
(see Q-26). It is the only occurrence of the module name anywhere in `docs/`,
and `docs/first-fit.md` is French-language content owned by `docs-english`
(task-7). Risk: Low.

### Q-29 — No stale path references in source comments. Observed. Risk: n/a (positive)

Every backticked `*.rs`/`*.md`/`*.json`/`*.wgsl`/`*.ps1` path inside `src/`
comments resolves to a real file (0 of them broken). Source-level documentation
tracks the tree.

---

## 9. Documentation drift and regression watch

### Q-30 — Documented test counts are stale. Observed. Risk: Low

- `docs/handoff.md:21`: "78 CPU tests, three opt-in GPU tests and native
  grabbing pass."
- `docs/handoff.md:31`: "76 CPU tests, two opt-in GPU tests and 37 native
  actions pass."

Measured now: **106** passed with `--no-default-features`; **106 passed + 3
ignored** with `--features desktop`. Both documented numbers are wrong by ~30 %.

**Proposed change:** replace with the measured command and count, or drop the
count entirely and cite the command — a count in prose rots, a command does not.
Risk: Low. Owner: `docs-english` (task-7). I will not edit it.

### Q-31 — Regression watch: `src/atelier/settings.rs` is unwired. Observed

348 lines with 7 unit tests; no `mod settings;` exists anywhere in the tree, so
it is not compiled and its tests cannot run. This is task-2 work in flight, not
a defect. **Standing duty:** when it lands, verify (a) `mod settings;` appears in
`src/atelier.rs`, (b) `cargo clippy --features desktop --all-targets -D warnings`
stays clean, and (c) its 7 tests actually execute under
`cargo test --features desktop` — which is only guaranteed if Q-16 is adopted.

### Q-32 — Regression watch: the smoke-target keys must not be translated. Observed

`src/atelier/authoring_smoke.rs:9-52` is a 41-entry `ACTIONS` table whose first
element of each pair is a smoke-protocol key. Several keys are still French —
`"Plume"`, `"Modifier"`, `"Coudre"`, `"Déplacer 3D"` — **deliberately**, per the
comment at `src/atelier/authoring_smoke.rs:5-8`. Two teammates are translating
UI strings right now (task-1, task-2). Translating a key instead of a label would
silently break the native smoke harness at
`src/atelier/authoring_smoke.rs:157` (`authoring_smoke_tick`). The matching
warning exists at `src/atelier/authoring_ui.rs:169`.

**Proposed change:** the Lead should state this to both translators explicitly.
Optionally pin the key set with an assertion so a rename cannot land silently.
Risk: Medium if ignored (`AGENTS.md:38` requires the harness to stay real).

---

## Prioritised backlog

Ordered by value per unit of risk. "Window" means it needs a refactor window and
an owner; "Round 1" means it was safe to apply immediately.

| # | Finding | Action | Risk | Needs |
|---|---|---|---|---|
| 1 | Q-16, Q-15 | Make the `desktop` CI job gating; add `cargo test --features desktop`; update the three stale caveats | Low | Lead decision (CI file) |
| 2 | Q-31 | Verify `settings.rs` gets wired and its tests run | Low | task-2 to land |
| 3 | Q-32 | Tell both translators the smoke keys are frozen | Low | Lead |
| 4 | Q-28 | ~~Fix 3 broken doc links~~ **DONE in round 1**; 1 wrong file name still to route | Low | `docs-english` for `docs/first-fit.md:57` |
| 5 | Q-30, Q-25, Q-27 | Doc-drift fixes | Low | `docs-english` (task-7) |
| 6 | Q-22, Q-07 | Measures: dependency-tree cost of `tracing`, cold build with/without `desktop` | Low | `register` cold-build table (pending) |
| 7 | Q-21, Q-23, Q-24 | `rust-version`, dev-profile note, POSIX check script | Low | refactor window |
| 8 | Q-02 | CI assertion that the CPU graph excludes `eframe`/`wgpu` | Low | refactor window |
| 9 | Q-05, Q-13 | Remove `add_scaled` (perf-approved); add the check-only colour bound; name the GPU tolerances; document the `same_physics` expect — **all four approved by `register`, zero measured cost** | Low | refactor window |
| 10 | Q-08, Q-09 | Shared `Triangle::from_rest`; shared energy helpers — Q-09 **downgraded** by `register` to a quality win with **medium** correctness risk (order-sensitive oracle) | **Medium** | refactor window |
| 11 | Q-18 | Add `seam_j` to `VbdMetrics` (or document its absence); a guides-contribution test | Low | refactor window |
| 12 | Q-04, Q-03 | Split `authoring::apply`; then `solver.rs` | Medium | refactor window |
| 13 | Q-10 | One shared check script used by CI and by contributors | Low | refactor window |
| 14 | Q-33 | Point the recorded CPU baseline at the engine the product ships, or label which engine each number came from | Medium | `register` (measurement) + window |
| 15 | Q-26, Q-11, Q-29 | Document the `#[path]` includes, the error convention, and keep comments honest | Low | `docs-english` coordination |

---

## Quality + efficiency agenda (co-owned with `register`, task-12)

> **Status: PROVISIONAL — my half only.** Bullets 1–5 below are `tf-cleanup`'s
> half. `register` replied during round 1 with **adjudications but not yet its
> bullets or its build table** (it is measuring from a pinned `git archive HEAD`
> snapshot, because the working tree's desktop build is broken mid-flight). The
> adjudications are folded in above: Q-09, Q-13 (colour bound + GPU tolerance
> names) and Q-05.
>
> **Merge contract for round 2:** `register` sends its 3–6 bullets; they are
> inserted here *verbatim*, this block is replaced by `Status: FINAL`, and the
> identical text is copied into `docs/PERFORMANCE.md`. Until then, do not quote
> the numbers in this section from another document, and do not treat the
> absence of a measurement bullet as a finding against `register` — its cold-build
> table is the dependency for bullet 2. One provisional number is already known
> and is *contended*, not clean: cold `--features desktop` at HEAD measured
> **216 s wall** (load average 2.6 before → 10.9 after), producing
> `cloth-fit` 20 223 728 B and `motion_probe` 7 413 200 B.

1. **Make the desktop job gating, and add `cargo test --features desktop`.**
   Proven at HEAD: the advisory job's commands exit 0, and
   `cargo test --features desktop` runs 106 tests, ignores the 3 GPU tests and
   finishes in 41.6 s with no GPU. Today 6 232 of 14 539 `src/` lines (42.9 %)
   are compiled only under `desktop` and CI executes **zero** tests in them. It
   is the largest coverage gain per second of CI time, and it is the prerequisite
   for any performance guard (Q-17).
2. **Retire or justify `tracing` + `tracing-subscriber`.** 15 of the 83 CPU
   packages (18 %) exist for one `tracing::debug!`
   (`src/solver.rs:718-724`), and the subscriber is installed only in
   `src/main.rs:47-51` — so the log is dead in `cloth-fit`, the documented
   primary entry point. `register` owns the cold-build and binary-size delta.
3. **Perf-regression coverage is structurally impossible today.**
   `--no-default-features` = 106 tests; `--features desktop` = the same 106 plus
   3 ignored. There is no timing, allocation or transfer-budget assertion in the
   suite, and item 1 is the prerequisite for adding one.
4. **Shared construction of a protected invariant.** The
   `Triangle { rest, inv_dm, area }` derivation is written twice
   (`src/model.rs:176-189`, `src/garment/polygon.rs:180-187`) although
   `docs/architecture.md:60` makes it a contract.
5. **[perf-sensitive] coordination rule both forces accept:** any change whose
   cost is not provably zero on a measured hot path waits for `register`'s
   before/after on the same scene. **Round-1 outcome of that rule — all four
   candidates adjudicated, none touched by me:**
   - `src/vbd.rs:210-226` — hot path, but **check-only is free**; approved on the
     strict condition that the *colouring itself* is not modified (any reorder
     changes trajectories bitwise). See Q-13.
   - `src/gpu_vbd.rs:362,370` — opt-in diagnostic, **free, approved**. See Q-13.
   - `src/blocksolve/sym_mat3.rs:67` — dead code is link-eliminated, no runtime
     cost; **approved on performance grounds**, removal is an API-surface call.
     See Q-05.
   - `src/solver.rs:346-367` vs `640-661` — **not on the default hot path**
     (reference solver only), so a quality win, not a performance win — and
     **medium** correctness risk because that energy is the finite-difference
     oracle. See Q-09.
6. **The recorded CPU baseline measures the wrong engine.** `scripts/bench-cpu.ps1:19`
   drives `marvelous-cloth --headless` (legacy Newton), while the shipped CLI
   default is VBD+OGC and the atelier's default is the hybrid GPU backend. Both
   forces agree this is a measurement/coverage gap; neither has changed it. See
   Q-33.

---

## Round 1 status

**Applied in round 1:**

- `docs/CODE-QUALITY.md` — this file. Created.
- `docs/clo3d-feature-catalog-earlier-pass.md` — **3 broken relative links fixed**
  (lines 28, 329, 337). This was outside my original declared write scope; I
  reopened task-11 and added the file to `writeScopes` only after `register`
  confirmed the file is not its scope. Post-fix: 0 broken relative links in real
  prose across the whole repository. Diff: 3 insertions, 3 deletions, no prose
  changed.

**Deliberately not applied, and why:**

- No `src/` change. Four feature teammates are writing
  `src/render.rs`, `src/atelier/board.rs`, `src/atelier/workspace.rs` and
  `src/atelier/icons.rs`; my own off-limits list adds `src/atelier.rs`,
  `src/atelier/settings.rs` and `src/atelier/view_controls.rs`. Every `src/`
  finding above is a refactor or touches a file under active edit.
- Q-05 (`add_scaled`) is the only dead-code removal proposed, and it fails the
  round-1 bar: it is `pub`, so `cargo` cannot confirm it unused. `register`
  approved it on performance grounds, but approval is not the same as
  compiler confirmation, so it stays in the backlog.
- Q-28's third link item — `docs/first-fit.md:57` naming a non-existent
  `fitting_editor.rs` — is **not** applied: that file is French-language content
  owned by `docs-english` (task-7). Routed, not clobbered.
- No `cargo fmt`, no commit, no push, per the charter.

**Commands run in round 1** (all from a clean tree at HEAD `16f6fc4`; the tree
became dirty afterwards, which is recorded above):

```
cargo check  --locked --no-default-features --all-targets -j 2   # exit 0, 3.3 s
cargo test   --locked --no-default-features -j 2 -- --test-threads=2   # 106 passed
cargo clippy --locked --no-default-features --all-targets -j 2 -- -D warnings   # exit 0
cargo clippy --locked --features desktop   --all-targets -j 2 -- -D warnings   # exit 0
cargo test   --locked --features desktop   -j 2 -- --test-threads=2   # 106 passed, 3 ignored
cargo tree / cargo metadata                                      # package counts
```

`RAYON_NUM_THREADS=2` and `-j 2` throughout, per `AGENTS.md:13`.

## Standing duty

This force does not expire with this task. Each round:

1. Re-pin to the new HEAD and re-measure the tables above.
2. Re-audit the round's diff for structural regressions: new god-functions,
   a numeric module acquiring a UI dependency, a second error convention, a
   duplicated derivation, a new `#[allow(dead_code)]`.
3. Verify Q-31 (settings.rs wired + tests running) and Q-32 (smoke keys intact).
4. Keep the *Quality + efficiency agenda* sections of `docs/CODE-QUALITY.md` and
   `docs/PERFORMANCE.md` byte-identical, and re-sync them whenever either force
   adds a bullet.
