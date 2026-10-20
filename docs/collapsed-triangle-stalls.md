# Collapsed-triangle stalls in the ruffled dress

19 October 2026. The owner supplied a newly authored dress and a marked issue
capture. This investigation reproduces apparent pins in the production VBD/OGC
solver. **The stall is not fixed.** A trial solver change was rejected and
reverted; production contact rules, damping, prediction and materials remain
unchanged.

## Evidence

The capture and the saved document have identical content except for the avatar
path. The untouched capture was copied before testing. It contains 5,126 cloth
vertices, 9,546 triangles, six panels, seven seams and no explicit pins. Seven of
the nine most restricted vertices belong to the user's painted region; the
other two belong to the same local neighborhoods.

Two current triangles have almost lost rank. Their rest triangles are ordinary
nondegenerate material triangles:

| Panel | Current/rest area | Minimum current altitude | Propagated OGC ball radius | Affected vertices |
| --- | ---: | ---: | ---: | ---: |
| Back | 0.000211093 | 2.69125 µm | 1.21106 µm | 5 |
| Lower ruffle | 0.00180163 | 16.9185 µm | 7.61331 µm | 4 |

The near pair in each neighborhood is between the opposite vertex of the
flattened triangle and the shared edge of its neighboring triangle. There is
also an EE witness in the back neighborhood. The feasible-region tests reject
forces on these features, but the distances still restrict all primitive
vertices. That distinction is intentional: dropping rejected force features
from the bounds would break the collision coverage argument.

The affected vertices have no incident contact energy terms in the initial
probe. Their small movement allowance is a geometric restriction, not an
explicit pin, friction or a direct sewing attachment. Sewing can load these
regions, but this capture does not show when or why the triangles collapsed.
The membrane energy remains finite at rank loss, and bending derivatives become
poorly conditioned as triangle area approaches zero. These are relevant model
and convergence limitations, not proof of a specific historical trigger.

## CPU and GPU replays

All continuations start from the same accepted state, use 16 iterations,
dt = 1/60 s and two CPU workers. The production GPU path is hybrid f32/f64.

- After 60 CPU steps the back group moves only 0.203–0.219 mm; the lower ruffle
  group moves 13.27–13.37 mm. The back restriction therefore survives a full-f64
  solve; this is not solely the previously fixed GPU rounding stall.
- After 120 baseline GPU steps the back group moves 0.030–0.148 mm and the
  lower ruffle group 0.776–0.777 mm. All nine still have fresh bounds below
  10 µm. The precision fallback visits five vertices on 116 steps and nine on
  four steps in the first diagnostic run.
- An isolated 120-step baseline run averages 17.70 ms per complete step, with
  P95 18.50 ms. This is about 56.5 physical steps/s, not 60 Hz or rendered FPS.
  The scene is partly stalled, so this is not a performance guarantee for a
  freely moving dress. The initial CPU/GPU diagnostic overlap is excluded from
  this timing claim.

The public CPU regression
`flattened_neighbor_face_restricts_motion_without_an_active_contact_force`
uses only two adjacent triangles, no avatar, seams or private geometry. It
shows that tiny bounds can exist with zero active contact force, while a
collective translation preserves separation. The test documents the conservative
bound contract; it is not a fix or permission to remove a pair.

## Rejected local-solve trial

A temporary f64 trial minimized the SPD quadratic model on the existing ball
using a scalar Lagrange multiplier, instead of radially clipping the Newton
proposal. The full nonlinear objective was still checked. After 120 GPU steps,
the back group had moved only 0.053–0.135 mm and the lower group 0.747–0.768 mm.
It did not release the folds. Both source and production executable were
restored to their pre-trial hashes. The trial executable remains in ignored
local evidence only. No extra damping, sleeping, positional repair, larger
exclusion or material-area floor was introduced.

## Instrumentation retained

Issue reports now include current/rest area ratio and minimum altitude for each
reported face, the twenty most compressed faces and a count below one percent
of rest area. The threshold is diagnostic only. These unsigned measurements do
not certify orientation or intersection. They are evaluated during report
export, not in simulation steps.

`seam_block_probe --baseline-only` runs the production solver through the
existing settle/pull/hold/release schedule without constructing a second
comparison branch. `--check-gpu-contacts` adds the existing full f64 detector
oracle at every step; timings with this flag are validation timings, not a
performance result. Controls use panel ID and material coordinates. The target
moves 40 mm over steps 120–179, holds through 209, then releases through 359.

On this dress, the single-baseline 360-step run moved the selected attachment
only 0.446 mm during the pull and 1.315 mm during release. It remains restricted;
passing the collision checks is not a motion-quality pass. Complete step times
averaged 20.34 ms during settling, 23.64 ms during pulling, 25.21 ms while holding
and 22.54 ms after release (22.21 ms over the run). These are one-run values on
an RTX 3090, with offline energy/contact audits excluded from the step timer.
They do not establish 60 physical steps/s or stable performance across workloads.

A separate 360-step pull/release with `--check-gpu-contacts` passed exact equality
of candidate counts, distances, balls, canonical contacts and incidences against
the CPU detector at every step. Both runs passed the fresh final intersection
diagnostic. The minimum observed fresh separation was 2.702 µm. These finite
precision checks retain their documented seam exclusions and tolerances; they
are not an exact continuous all-pairs certificate.
The complete serialized trajectory is byte-identical with and without the CPU
oracle, so enabling that check did not alter the observed motion.

```powershell
# Read-only report; CPU, no time advancement
cargo run --release --locked --no-default-features -j 2 --example issue_probe -- CAPTURE/project.clothcraft.json output/shape-reports

# Hardware use must be explicitly authorized for the session
target/release/examples/seam_block_probe.exe --project CAPTURE/project.clothcraft.json --gpu --baseline-only --steps 360 --panel panel_3 --point 0.618 0.012 --output output/dress-release

# Separate validation run; do not quote its timings as throughput
target/release/examples/seam_block_probe.exe --project CAPTURE/project.clothcraft.json --gpu --baseline-only --check-gpu-contacts --steps 360 --panel panel_3 --point 0.618 0.012 --output output/dress-release-oracle
```

## Follow-up

The subsequent [directional-contact implementation](directional-contact.md)
replaces only the most restrictive near-pair balls with verified separating
half-spaces, retaining residual balls and the query envelope. The measurements
and rejected experiment above describe the earlier isotropic investigation.

## Next solver experiment

1. Reproduce entry into rank loss on a small public cloth fixture, measuring
   area, forces, local conditioning and contact restrictions before collapse.
   The saved dress is a continuation test, not that missing history.
2. Evaluate directional feasible regions or a coupled feasible translation
   against the two-triangle case, then cloth pull/release. Prove coverage of
   unseen pairs, VF in both directions, EE and boundary features before enabling
   any less restrictive region. This is a contact-algorithm change, not a scalar
   tolerance adjustment. See the [solver review](solver-landscape-2026-10.md).
3. Separately evaluate prevention of material rank loss. An area/strain barrier
   changes the constitutive model and needs derivatives, units, calibration and
   resolution tests. Do not silently add it as an anti-jitter force or rebuild
   rest geometry from the draped state.
4. Require ordinary scenes, newly compressed folds, loaded seams and matched
   pull/release to pass collision and motion checks. Compare complete step
   latency on moving cloth; a frozen fast result fails the acceptance gate.

## Validation of the retained changes

- CPU library, OGC and issue-report tests: 31 passed with two test threads,
  including the new rigid-transform/area diagnostic and public flattening case.
- CPU Clippy for the library, replay example and affected integration tests:
  passed with warnings denied. Formatting of edited Rust files passed.
- The baseline replay example built in Release with desktop support. The
  baseline and CPU-oracle GPU pull/release runs both completed all 360 steps.
- The final `cloth-fit` Release executable built successfully with the new
  report fields and unchanged production physics. No editor window was opened.
- The original document and issue project retain their initial SHA-256 hashes.
  The rejected solver experiment was removed before the final build; only
  diagnostics and the offline replay harness changed in this investigation.

Private captures, trajectories and candidate binaries stay under ignored
`output/ruffled-dress-benchmark/issue-EcJ1OC/`. No user document was overwritten,
and no commit or push was requested for this investigation.
