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

All `file:line` citations and line counts are pinned to **`16f6fc4`**
("Make the interface plain English, widen the CLO3D register, ship DRAW-01")
unless a finding says otherwise.

**Update, after this audit was written:** that revision is now `HEAD~1`. The Lead
landed `db46440` ("Add view navigation, an icon set, preferences with a dark
theme, and the quality audit"), which contains the first version of this file and
the three CLO3D doc-link fixes, and which also brought `src/atelier/view_controls.rs`,
`src/atelier/settings.rs` and `src/atelier/icons.rs` into the tree. So:

- **`16f6fc4` is the measurement pin; `db46440` is current HEAD.** Every count in
  this document describes `16f6fc4`. Nothing here has been re-measured at
  `db46440`.
- **Re-pinning is deliberately deferred to the *next* HEAD, not `db46440`.** The
  Lead ruled on this explicitly, and the reasoning is worth keeping because it is
  not the obvious answer: the counts here are **structural** — line totals, the
  desktop-only share, dependency-package counts — and that is exactly what the
  next commit changes. Pinning to `db46440` now would guarantee a third stale set
  the moment the pending wiring and docs commit lands. The trigger is therefore:
  **the Lead lands the wiring and docs commit, then this file re-pins to that
  HEAD** — which is also the point at which the live tree becomes worth building
  again, since the feature writers will have stopped.
- **This is a deliberate asymmetry with `register`, not a disagreement.**
  `register` proceeds against a `git archive db46440` snapshot now, because for
  performance the re-pin is documentation hygiene rather than re-measurement — the
  solver is byte-identical between the two revisions. For this file it *is* a
  re-measurement, so it waits. A reader should not read one of us as lagging.
- One distinction from that decision is worth preserving even though it was not
  acted on here: `git archive db46440` is a snapshot of a **committed** tree, so
  it cannot go red. The only cost of an earlier re-pin would have been documentary
  confusion, never a broken build.
- The conclusions are expected to hold, because they are structural, but that is
  a prediction to verify, not a fact to assert.
- **Pending at the re-pin, in one bundle:** line counts and the desktop-only share
  (Q-15, Q-20); dependency-package counts (Q-22); and the desktop lint plus the
  `cloth-fit` bin test target from the live tree — the last two were answered by
  the Lead's manual gate (see Q-31) but have never been reproduced from CI.

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

**Consequence, stated plainly:** line numbers in this document are valid at
`16f6fc4`. For `src/render.rs`, `src/atelier.rs`, `src/atelier/board.rs`,
`src/atelier/authoring_ui.rs` and `src/atelier/workspace.rs` the tree has already
diverged, so a citation there identifies the code at `16f6fc4`, not the code you
will find today. Re-resolve by symbol name, not by line number, until the feature
work freezes and this file is re-pinned.

Pinned size at `16f6fc4`: **55 files, 14 539 lines** in `src/`, plus 3 120 lines
in `tests/` (3 084 in the 17 integration targets, 36 in the shared
`tests/common/mod.rs`) and approximately 927 lines in in-source `#[cfg(test)]`
modules.

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
| Desktop tests are GPU-free | `cargo test --locked --features desktop -j 2 -- --test-threads=2` | 106 passed, **3 ignored**, 0 failed, 41.6 s ⚠ upper bound |
| Dependency graph size | `cargo metadata --locked [--no-default-features] --format-version 1` | 83 packages CPU / 174 with `desktop` |
| Line counts | `git show HEAD:<path> \| wc -l` | see tables below |
| No broken doc links | a relative-link check over every `*.md` (ignoring code spans) | 0 broken in real prose |

### ⚠ Measurement environment — read before quoting any timing

Every wall-clock figure in this document was taken on a **memory-exhausted,
swap-thrashing host**, verified at the time of writing:

```
$ sysctl vm.swapusage
vm.swapusage: total = 12288.00M  used = 11283.69M  free = 1004.31M  (encrypted)
$ uptime
load averages: 44.64 61.71 35.42      # with no cargo or rustc process running
```

`register` independently observed load 60–130 and 11.3 GB of 12.3 GB swap during
its own cold builds, and reports that its 216 s cold-desktop figure is therefore
*double-contended* (build load plus swap). Consequences for this document:

- **Timings are upper bounds, not measurements.** The 41.6 s for
  `cargo test --features desktop` and the 3.3 s for the CPU check are "this is
  what it cost once on a thrashing machine", not a baseline. `register` owns the
  real table and will re-run cold builds in the quiet window the Lead is
  arranging, recording load and swap per row.
- **The substantive claims are unaffected.** Exit codes and pass/ignore/fail
  counts are deterministic and do not depend on load: the desktop feature
  compiles, lints clean, and its tests pass with the 3 hardware tests ignored.
  That is what Q-16 turns on, and it is not a timing claim.
- **Do not size CI budgets from this file.** See the quality + efficiency agenda.

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
  → 106 passed; 3 ignored; 0 failed; 41.6 s   # 41.6 s is an UPPER BOUND, see the
                                              # measurement-environment warning above
```

No GPU is touched: the three hardware tests are `#[ignore]`, so
`cargo test --features desktop` is safe for CI and does not violate
`AGENTS.md:22` ("No GPU test may be added to the suite that runs by default").

`src/atelier/settings.rs` — 348 lines with 7 unit tests, added by task-2 — is
proof that the team is already writing desktop unit tests that cannot run
anywhere.

**The cost of a non-gating job, as an incident rather than a percentage.** When
`db46440` landed it added or rewrote seven desktop-only files — `board.rs` +338,
`icons.rs` +848, `settings.rs` +357, `view_controls.rs` +201, `render.rs` +673,
`workspace.rs` +84, `atelier.rs` +7 — roughly **2 500 lines of code that no
enforced job compiles**.

**Correction to my own earlier wording.** I first wrote that nothing had verified
`db46440`'s desktop build. That was wrong, and the Lead corrected it: the commit
**was** gated on a local manual check run on the exact tree that became
`db46440`, whose output was:

```
fmt=yes  desktop_errors=0  desktop_warnings=28  cpu_failures=0  bin_tests=13 passed  bin_failures=0
```

So the accurate statement is **no continuous signal**, not no verification — and
that distinction is exactly why the CI change is worth more than any manual check.
A manual gate verifies one tree, once, in one person's session; it leaves nothing
behind that would report a regression tomorrow. Right now there genuinely is no
signal, because the tree has moved on and is red from later uncommitted work.

Two useful facts fall out of that gate. First, `bin_tests=13 passed` is the
**empirical proof of Q-31(c)** that I could not otherwise obtain: 6 tests in
`src/atelier/icons.rs` plus the 7 in `src/atelier/settings.rs` — both counted at
`db46440` — are exactly 13, so the `settings.rs` tests really do execute under
`--features desktop`. Second, `desktop_warnings=28` means the advisory job's
`cargo clippy --features desktop --all-targets -- -D warnings` step **would fail
on `db46440`**, so the job cannot simply be flipped to gating: the 28 warnings
must be cleared first. The Lead is landing that with the wiring commit that drives
the count to zero, which is the right order — fix the code, then tighten the gate.

**Proposed change (accepted by the Lead as overdue; the single highest-value change):**

1. **Clear the 28 desktop warnings first.** Until then the job's existing
   `cargo clippy --features desktop --all-targets -- -D warnings` step fails on
   `db46440`, so the job cannot be made gating as-is. Rewording the gate to pass
   it would be the wrong fix; the Lead is landing the cleanup in the same commit
   as the wiring that produces it.
2. Drop `continue-on-error: true` from the `desktop` job in
   `.github/workflows/cpu.yml`.
3. Add to that job: `cargo test --locked --features desktop -j 2 -- --test-threads=2`.
4. Update the three places that carry the stale caveat in the same pull request:
   `ROADMAP.md:17` (the P0.5 row), `ROADMAP.md:125` (the caveat bullet) and
   `docs/handoff.md:9` — "a **consultative** `desktop` job (`continue-on-error`,
   to be validated on Linux)". The last is `docs-english`'s file: coordinate.

Risk: Low. Cost was 41.6 s with warm dependencies, but that figure is an
**upper bound** on a swap-thrashing host (see the measurement-environment warning
above) — the claim that matters here is pass/ignore/fail, not the clock. Note the
job currently uses `--bin cloth-fit --bin marvelous-cloth` for `check`;
`--all-targets` on the clippy step already covers tests and examples.

### Q-17 — No test asserts a budget. Observed. Risk: Medium

There is no timing, allocation, transfer-size or iteration-count *budget*
assertion anywhere in `tests/`. `ROADMAP.md:129` acknowledges the equivalent gap
for rendering ("The UI has no automated visual regression test"). This is
`register`'s lane; recorded here because it is a test-structure gap, and because
item Q-16 is its prerequisite: a perf guard has no home to live in today except
the CPU targets.

**`register` has now measured the noise floor, and it constrains what such a test
could ever assert.** Re-running the entire `16f6fc4` CPU matrix a second time on
the same binary, on an untouched tree, it measured:

| Metric | mean \|Δ\| | max \|Δ\| |
|---|---:|---:|
| CPU ms/step | 4.6 % | 10.8 % |
| single-run step P50 | 6.7 % | 20.3 % |
| peak RSS | 40.7–58.3 MB absolute movement | — |

Its standing rule, which I adopt: **no round-2 performance claim below ~11 % on
CPU time or ~20 % on a single-run P50 is a result.** The quality-side consequence
is the important one here — **any budget assertion in `tests/` must sit above that
noise or it will flake, and a flaky budget test is worse than none.** So a
perf guard cannot be written as `step_ms < X`; it needs either a paired
before/after comparison in the same run, a ratio against a baseline taken on the
same host, or a documented margin above the measured floor. Whoever implements
Q-17 must read `docs/PERFORMANCE.md` §1 first.

### Policy adopted by the Lead: no threshold-style budget in `tests/`

The Lead adopted the paragraph above as **repository policy**, and generalised it
past performance, which is why it is recorded as policy rather than as a finding:

> **No `step_ms < X` style budget in `tests/`.** A performance guard must be a
> same-run paired comparison, a ratio against a same-machine baseline, or carry a
> documented margin above the measured floor.

The generalisation is the durable part, and it is worth stating in full because it
explains *why* a generous-looking threshold is not a compromise: **an assertion
that can fail on an idle machine is a liability, because the team's response to a
flaky test is to ignore it — and then it stops guarding anything.** A test that
everyone learns to re-run is worse than no test, since it also spends the trust
that the rest of the suite needs.

Two consequences for whoever picks up backlog item 21:

1. Write it as a **paired comparison**, not a threshold — the same run must
   produce both sides, so machine state cancels.
2. If an absolute number is unavoidable, it must carry its **measured floor and
   margin in the comment**, so the next person can tell a real regression from
   host noise instead of re-deriving `register`'s table.

This also retroactively settles Q-16: if CPU time swings ±20 %, a 41.6 s figure
**cannot** be a budget, which is precisely why that argument had to rest on
pass/ignore/fail. That reasoning is now load-bearing rather than rhetorical.

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

One precision on the middle row, since `register` listed it among the non-gated
targets: `tests/vbd_shader.rs` does run under `--no-default-features`, but it
parses and validates `src/vbd.wgsl` on CPU through `naga`; it does **not** execute
the solver. It is shader-source validation, not engagement coverage, so it is not
counted above.

This is a deliberate correction of `register`'s original phrasing, which it has
reviewed and accepted. It wrote that "CPU tests are green currently says nothing
about the solver the CLI ships by default"; as first stated that was too strong.
The CPU suite **does** exercise VBD+OGC on CPU, so it is not silent about the
shipped CLI path. What is silent is the **GPU** engine the atelier ships, and what
is misleading is the benchmark script. The engine mismatch, not the test suite, is
the finding.

`scripts/bench-cpu.ps1:19` drives `target/release/marvelous-cloth --headless`,
i.e. the **legacy Newton** engine — while `docs/handoff.md:9` and
`AGENTS.md:5` describe the shipped default as VBD+OGC, and the atelier's default
as the hybrid GPU backend. So the script that produces the recorded CPU numbers
(`bench-cpu.ps1` feeding `output/cpu-suite/suite.json`) benchmarks neither the
CLI default nor the GUI default.

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
build-time headroom.

**`register`'s measured build matrix (`docs/PERFORMANCE.md` §5)** — pinned
`git archive 16f6fc4`, `--locked -j 2`, swap held at 11.3 GB of 12.3 GB for every
row, so **every wall figure is an upper bound**:

| Directories measured | wall | user | sys | peak RSS | load 1m before → after |
|---|---|---|---|---|---|
| Cold `--features desktop` (`cloth-fit` + `motion_probe`) | 238.1 s | 351.0 s | 25.5 s | 670 MB | 55.1 → 16.6 |
| Cold `--no-default-features` (`cloth-fit` + `marvelous-cloth` + `motion_probe`) | 53.5 s | 95.1 s | 5.9 s | 686 MB | 13.2 → 8.8 |
| Incremental, no-op (same target dir, same command) | 0.42 s | 0.08 s | 0.12 s | — | ~9 |
| Incremental, crate-only (`cargo clean -p marvelous-cloth`) | 21.6 s | 34.8 s | 1.7 s | — | ~9 |

Release binary sizes at the same revision: `cloth-fit` **20 219 984 B** with
`desktop` vs **2 465 552 B** without (8.2×); `motion_probe` 7 413 200 B vs
2 174 560 B; `marvelous-cloth` (not built desktop) 2 553 168 B. The load-robust
conclusions are the **ratios**: `--no-default-features` builds cold **4.4×**
faster and produces a CLI binary **8.2×** smaller.

`register` also re-measured cold desktop under *less* contention (load 55 → 17)
and got **238.1 s — slightly worse than the earlier 216 s**, and asks that
neither be presented as a clean figure. Noted; I have done so.

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

### Q-35 — A compiled helper is committed, and `.gitignore` cannot remove it. Observed. Risk: Low

Raised by `register`, which created the file and self-reported it. Verified
state at the time of writing:

- `scripts/perf/libmalloc_count.dylib` — a 34 328 B Mach-O compiled from
  `scripts/perf/malloc_count.c` — **is tracked in `db46440` and still in HEAD**
  (`git ls-tree -r --name-only HEAD | grep '\.dylib$'` returns it).
- It is the **only** compiled artifact in the index: the same check for
  `*.dylib|*.so|*.o|*.a|*.rlib|*.dSYM|__pycache__` over `git ls-files` returns
  nothing else. So this is an isolated slip, not a pattern.
- The `.gitignore` rule `register` asked for **already exists** — lines for
  `scripts/perf/*.dylib`, `scripts/perf/*.so` and `__pycache__/` are present. Its
  second request is therefore satisfied.
- `git status` shows the worktree copy deleted as an **unstaged** deletion
  (` D`), so the blob is still in the committed tree and will survive unless the
  deletion is included in a commit.

**The generalisable point:** `.gitignore` prevents *new* untracked files from
being added; it never untracks a path already in the index. Adding the rule is
necessary but not sufficient — the removal must be committed.

**Proposed change:** include `scripts/perf/libmalloc_count.dylib`'s deletion in
the next commit (`git rm --cached` if the worktree deletion is not picked up).
The `.c` source and the documented build command stay, which is right: the helper
is reproducible, so the binary need not be. Risk: Low. Owner: whoever makes the
next commit — I must not commit.

**Lead decision, recorded:** `git rm --cached scripts/perf/libmalloc_count.dylib`
goes into the next commit. The Lead also supplied the mechanism for how the blob
survived the ignore rule — the deletion was left *staged-but-lost* when the tree
was reset to unstage the red work. That is precisely the failure mode this finding
describes: a staged deletion is not a deletion until it is committed.

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

### Q-31 — Regression watch: `src/atelier/settings.rs` wiring. Observed — **RESOLVED**

At `16f6fc4`: 348 lines with 7 unit tests, and no `mod settings;` existed anywhere
in the tree, so it was not compiled and its tests could not run. That was task-2
work in flight, not a defect.

**Standing-duty re-check at `db46440`, all three steps RESOLVED.**

**(a) Wiring.** `src/atelier.rs:32-33` now declares
`#[path = "atelier/settings.rs"] mod settings;` unconditionally,
`src/atelier/settings.rs` is tracked, and it contains `#[cfg(test)] mod tests` at
`:233` with exactly 7 `#[test]` functions. Because `src/atelier.rs` is included by
the `cloth-fit` binary (`src/bin/cloth-fit.rs:9-11`) under `feature = "desktop"`,
those 7 tests are part of that bin's test target by construction.

**(b) Lint and compile.** The Lead's pre-commit gate on the exact tree that became
`db46440` reports `desktop_errors=0` and `cpu_failures=0`, with
`desktop_warnings=28`. So the tree compiles; it is not warning-clean, which is why
the advisory clippy step (`-D warnings`) cannot be made gating until those 28 are
cleared. See Q-16.

**(c) The tests execute.** `bin_tests=13 passed  bin_failures=0` from the same
gate, and 6 (`src/atelier/icons.rs`) + 7
(`src/atelier/settings.rs`) = 13. That is direct evidence the 7 tests run under
`--features desktop` — answering the question with a number rather than an
inference.

**My own attempt, kept because it produced a finding.** Before the Lead's gate
reached me I tried the check from the live tree in a genuine quiet window
(swap 9.75 GB, load 6.3, no cargo running) and it **failed to compile**:

```
$ cargo test --locked --features desktop -j 2 --bin cloth-fit -- --test-threads=2
error[E0599]: no method named `snap_geometry_for` found for struct `board::Board`
    --> src/bin/../atelier/board.rs:1365:26
error[E0599]: no associated function or constant named `default` found for struct
              `PolygonPattern`          --> src/bin/../atelier/board.rs:2676:63
error: could not compile `marvelous-cloth` (bin "cloth-fit" test) due to 2 previous errors
```

**Both error sites are uncommitted work, and this is provable rather than
assumed:** `HEAD:src/atelier/board.rs` is **1 130 lines**, so lines 1365 and 2676
cannot exist in it; the working tree's `board.rs` was **2 680 lines** (+1 606) when
the compiler ran, and `PolygonPattern` has no `Default` impl at HEAD *or* in the
tree yet. This is `src/atelier/board.rs` mid-refactor by a feature teammate —
Q-34's prediction, observed once more, now with a precise error to cite.

The Lead confirmed the attribution: `board.rs` is being edited **right now by
`ui-editor` for the drafting rework**, which is why the line count moved
2 680 → 2 668 while this was being written up. So `snap_geometry_for` and
`PolygonPattern::default()` are **transient mid-edit symbols, not a defect in
`db46440`** — the same class of event as the earlier `board.rs` parse error, and
further evidence for Q-34 rather than against the commit.

*Caveat on those line numbers:* they are a snapshot. `board.rs` had already shrunk
from 2 680 to **2 668 lines** by the time this paragraph was written, so a reader
re-checking line 2676 today will not find it. That is the point rather than a
defect — resolve these by symbol (`snap_geometry_for`, `PolygonPattern::default`)
and not by line, exactly as the revision-pin section says.

**Consequence that is bigger than the 7 tests:** `db46440` added or rewrote seven
desktop-only files — `board.rs` +338, `icons.rs` +848, `settings.rs` +357,
`view_controls.rs` +201, `render.rs` +673, `workspace.rs` +84, `atelier.rs` +7 —
roughly **2 500 lines of code that no enforced job compiles**. The Lead's manual
pre-commit gate did verify that tree once (`desktop_errors=0`,
`bin_tests=13 passed`), so it is **not** unverified — but there is **no continuous
signal**, and the only candidate is the `continue-on-error` job that cannot fail.
That is Q-16's argument stated as an incident rather than a percentage.

**Round-2 action:** the Lead's gate has now answered (b) and (c) above; only the
`desktop_warnings=28` cleanup and the CI change remain. If a future round needs to
re-check from the live tree, do it from a `git archive HEAD` snapshot (the pattern
`register` uses) rather than while four writers are mid-refactor.

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

**Standing-duty re-check at `db46440`: the protocol is INTACT.** `const ACTIONS`
still declares 41 entries and still contains 41; the accented protocol keys
(including `"Déplacer 3D"`, twice) survive; and `smoke_target` call sites still
exist across six files (`authoring_smoke.rs`, `authoring_ui.rs`, `board.rs` 11×,
`fabrics_ui.rs`, `gizmo.rs`, `seams_ui.rs`). The translations did **not** rename
a key. Risk at `db46440`: Low. It stays a watch item because the risk is silent —
a future rename breaks the harness with no compile error.

### Q-34 — HEAD is green; the working tree is intermittently red. Observed, operational. Risk: Medium

`register` reported a failing
`cargo test --features desktop --bin cloth-fit --no-run` in a ~45 s retry loop
during round 1. I could not reproduce it, and we resolved the disagreement:
**there is no failure at HEAD.** The cause was that `src/atelier/board.rs` did not
parse while it was mid-edit (the Lead confirmed this), and `register`'s loop was
building the dirty tree.

Both observations are correct and compatible:

- **HEAD `16f6fc4` is green** on every command in the table above — verified
  independently by both forces.
- **The working tree is intermittently red** while up to five teammates rewrite
  `src/render.rs`, `src/atelier/board.rs`, `src/atelier/workspace.rs`,
  `src/atelier.rs` and `src/gpu_bench.rs` and add `icons.rs`, `settings.rs` and
  `view_controls.rs`. A non-parsing intermediate state is expected, not a
  regression.

**Proposed change (needs the Lead):** state this explicitly so a future round
does not re-litigate the same false alarm, and prefer a `git archive HEAD`
snapshot for any measurement or verification that must be reproducible (`git
stash` is discouraged here: it is disruptive with four concurrent writers). The
counter-risk is a real failure being dismissed as "just the dirty tree"; the
mitigation is that every claim in this document is pinned to a revision and HEAD
stays green. Risk: Medium.

---

## Prioritised backlog

Ordered by value per unit of risk. "Window" means it needs a refactor window and
an owner; "Round 1" means it was safe to apply immediately.

| # | Finding | Action | Risk | Needs |
|---|---|---|---|---|
| 1 | Q-16, Q-15 | Make the `desktop` CI job gating; add `cargo test --features desktop`; update the three stale caveats | Low | Lead decision (CI file) |
| 2 | Q-31 | `settings.rs` is now wired (`src/atelier.rs:32`); still verify its 7 tests execute under `--features desktop` | Low | quiet window (deferred: host swap-thrashing) |
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
| 16 | Q-34 | Tell the team HEAD is green and the dirty tree is intermittently red; pin measurements to a revision | Medium | Lead |
| 17 | env | Re-measure every timing in this file once the host stops swap-thrashing | Low | `register` + quiet window |
| 18 | Q-31 (b)(c) | ~~Verify the desktop lint + the 7 `settings.rs` tests~~ **DONE** — the Lead's pre-commit gate gives `desktop_errors=0` and `bin_tests=13 passed` (6 `icons.rs` + 7 `settings.rs`) | Low | closed |
| 19 | Q-35 | Include the `libmalloc_count.dylib` deletion in the next commit (`.gitignore` is already correct) | Low | next committer (Lead accepted) |
| 20 | Q-16 | **Clear the 28 desktop warnings**, then flip the `desktop` job to gating — the warnings are why the gate cannot be tightened yet | Medium | Lead (in flight) |
| 21 | Q-17 | Any perf guard **written as a paired comparison, not a threshold** (Lead policy): same-run both sides, or an absolute number carrying `register`'s measured floor (~11 % CPU, ~20 % P50) and margin in the comment | Medium | optimisation window, with `register` |

---

## Quality + efficiency agenda (co-owned with `register`, task-12)

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

**Applied in round 1, continuation (after `register`'s measurements landed):**

- The **shared quality + efficiency agenda is now FINAL** and contains both
  halves: my 6 bullets and `register`'s 6 bullets verbatim, plus a pointer to
  where the measured numbers live. No longer provisional.
- `register`'s **measured build matrix** is recorded in Q-22 with load/swap
  annotations, and the load-robust ratios (4.4× cold-build, 8.2× binary size)
  are cited in the agenda.
- **Q-09, Q-13, Q-05** carry `register`'s adjudications.
- **Q-16** gained the incident that makes its argument concrete: `db46440`
  shipped ~2 500 desktop-only lines that a **manual gate** verified once, with
  **no continuous signal** — because the only job that would report a regression
  is the one that cannot fail.
- **Q-31** re-checked: **fully resolved.** Wiring confirmed, and the Lead's
  pre-commit gate supplies the empirical answer I could not get —
  `desktop_errors=0` and `bin_tests=13 passed` (6 in `icons.rs` + the 7 in
  `settings.rs`), so those tests really do execute under `--features desktop`. My
  own attempt from the live tree is kept in the finding, with its exact compiler
  errors, because it produced the Q-34 evidence.
- **Q-32** re-checked and **passing**: the smoke protocol survived the
  translations intact.
- **Q-33, Q-34, Q-35** added: the wrong baseline engine, the HEAD-green /
  tree-red distinction, and the committed build artifact.
- **Q-17 extended** with `register`'s measured **noise floor** (mean 4.6 % /
  max 10.8 % on CPU ms/step; 6.7 % / 20.3 % on a single-run P50), which is the
  constraint any future budget assertion must clear. The shared agenda block is
  **unchanged** by this addition — hash still `5585bcf7…dfb3` — so no re-sync was
  needed.

**Self-corrections made in this continuation, recorded rather than quietly fixed:**

1. **I claimed nothing had verified `db46440`'s desktop build.** Wrong — the Lead
   gated it locally with `desktop_errors=0`, `bin_tests=13 passed`. Corrected to
   **no continuous signal** in Q-16, Q-31 and the shared agenda. The distinction
   *is* the argument for CI, so the correction strengthens the finding.
2. **I implied `register`'s failing retry loop was spurious.** It was real: it
   reported a genuine dirty-tree error I never saw because I never built the dirty
   tree. Corrected in Q-34 and acknowledged directly to `register`.
3. **My line-number citations from that failed build went stale while I wrote
   them up** — the file is under active edit. Flagged in place, with an
   instruction to resolve by symbol instead.

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
became dirty afterwards, which is recorded above). Every exit code and count
below is deterministic. **Every wall-clock figure is an upper bound** taken while
the host was swap-thrashing — see the measurement-environment warning:

```
cargo check  --locked --no-default-features --all-targets -j 2   # exit 0, 3.3 s*
cargo test   --locked --no-default-features -j 2 -- --test-threads=2   # 106 passed
cargo clippy --locked --no-default-features --all-targets -j 2 -- -D warnings   # exit 0
cargo clippy --locked --features desktop   --all-targets -j 2 -- -D warnings   # exit 0
cargo test   --locked --features desktop   -j 2 -- --test-threads=2   # 106 passed, 3 ignored
cargo tree / cargo metadata                                      # package counts
# * contended timing; do not use as a baseline
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
