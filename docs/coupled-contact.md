# Coupled material, sewing and hard floor contact

The local inequality experiment now has a coupled, deforming cloth test.
Two small panels use the **existing membrane, bending and sewing energies**,
with either matching seam lengths or 10% easing. The hard-contact law is
still experimental. Nothing in the editor's VBD/OGC path is replaced.

## What changed

`solver_lab/coupled_patch.rs` assembles the existing scalar material/sewing
factors, their exact gradient and a dense Gauss–Newton metric, including
off-diagonal coupling between vertices. Inertia makes the search metric
positive definite. Rest coordinates and constitutive parameters are unchanged.
The metric approximates curvature; it is not the exact material Hessian.

`solver_lab/dense_contact.rs` solves inequalities involving multiple moving
vertices. A test with two touching, mobile coordinates demonstrates why this
matters: the collective solution translates both, while independent feasible
point updates can block the loaded coordinate. The dense kernel is limited to
192 degrees of freedom and is a small-system research oracle.

Two dual methods solve the same quadratic subproblem:

- **Coordinate:** update one nonnegative contact multiplier at a time.
- **ActiveBlock:** after a coordinate sweep, jointly solve the positive
  multipliers through their Schur matrix. A step to the nonnegative boundary
  handles negative target multipliers. Singular active blocks fall back to
  coordinate ascent, without adding compliance or regularization.

The second method changes the numerical search, not the objective or contact
law. It is not a full AVBD implementation or a production solver.

## Dynamic experiment

`examples/coupled_contact_probe.rs` compiles two rectangular panels, 12 vertices
and eight triangles total. The panels lie above a planar floor, separated by
an explicit 1 mm hard clearance. They use the original seam quadrature and
exclusions. A corner is pulled 20 mm sideways and 15 mm upward, then released
either during motion or after a hold. Each scenario runs 360 steps at 1/60 s.

Each nonlinear iteration:

1. Assembles the complete inertial/material/sewing objective and current grab.
2. Solves all floor inequalities with the full coupled search metric.
3. Retains the original OGC bounds using a common step fraction for all
   vertices, then backtracks against the actual nonlinear objective.
4. Checks the full current force balance and complementarity before claiming
   convergence. A small search displacement alone is insufficient.

Every completed step also runs the original intersection/degeneracy diagnostic.
Floor clearance holds over each straight accepted path because all vertices
remain above the plane. Self-collision retains the original OGC guards, but
there is **no coupled hard self-contact reaction in this prototype**.

No velocity filter, sleep, extra damping, material softening, rest-shape update
or garment-specific exception is used. Backward Euler retains its inherent
numerical dissipation.

## Evidence

The eight matched runs complete 2,880 steps without an unconverged step or
reported geometry failure. All current force residuals are below 7.11e-8 N,
with complementarity below 2.05e-13 J. Floor height is at least
0.999999990 mm for the coordinate control and 1 mm to floating-point precision
for the block method.

| Seam / release | Coordinate, ms/step | Active block, ms/step | Observed ratio |
| --- | ---: | ---: | ---: |
| Matching / moving | 23.59 | 1.34 | 17.6× |
| Matching / held | 22.59 | 1.28 | 17.7× |
| 10% easing / moving | 50.19 | 2.83 | 17.7× |
| 10% easing / held | 43.71 | 2.17 | 20.1× |

These are one sequential sample per scenario/method, from the **same CPU
development binary**, on a 12-vertex research scene. Timing includes objective
assembly, collision guards, per-step diagnostics and trace construction;
initialization and final JSON serialization are excluded. These are not
whole-dress FPS or a production speedup.

The block method needs about 7–14 dual sweeps per complete step, versus
2,840–6,256 for the coordinate method. No singular-block fallback occurs in
these dynamic fixtures. Separate unit tests retain duplicated constraints,
infeasible constraints and changing active sets.

Maximum differences over the matched trajectories:

- Position: 3.65e-9 m.
- Velocity: 3.67e-8 m/s.
- Largest post-release mechanical-energy rise: 1.44e-15 J.

The eased seam retains approximately **0.333 mJ of elastic energy** after
release. This is prestress, not an error to erase. After a held release,
velocity changes over the final second have RMS about 1.1–1.4e-7 m/s per step.
Moving-release runs retain translation/rotation and have larger velocity
changes; continued motion without friction is expected. Decreasing energy
alone does not prove the absence of jitter.

See [the measured results](coupled-contact-validation.json). Raw trajectories,
binary and source snapshot are under ignored `output/coupled-contact-20261020/`.

## Validation and remaining work

Five dense-kernel/assembly tests cover shared motion, the original 3D QP
reference, infeasibility, coupled-reaction acceleration, duplicates, 32 seeded
active-set problems, exact production energy, finite-difference forces and
internal force/torque balance. The targeted suite has 37 passing CPU tests,
including the earlier contact/energy/release regressions.

The next necessary gate is **coupled deforming self-contact**: both sides of a
VF/EE pair must participate, with feature changes, original seam exclusions
and swept-geometry validation. Narrow gaps, folds and contact-set rank changes
must be tested. The dense oracle cannot scale directly to the dress; a sparse
or matrix-free solve must match it before timing the real garment.

No GPU, new dress trajectory, editor replacement, commit or push was performed.
The production jitter issue remains open.

## Reproduction

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test dense_contact --test contact_qp -- --test-threads=2
cargo build --locked --no-default-features -j 2 --example coupled_contact_probe
target/debug/examples/coupled_contact_probe.exe --active-block --output output/coupled-contact-20261020/active-block.json
target/debug/examples/coupled_contact_probe.exe --output output/coupled-contact-20261020/coordinate.json
python scripts/summarize-coupled-contact.py output/coupled-contact-20261020/coordinate.json output/coupled-contact-20261020/active-block.json --output docs/coupled-contact-validation.json
```
