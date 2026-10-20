# Energy accounting and release behavior

Status: CPU research and opt-in diagnostics, 16 October 2026. No production physics change, GPU run, window launch or executable replacement. The objective-based prediction rejection remains removed. The owner's observations of sticking and jerky release invalidate that experiment despite its lower settled-motion RMS.

## The criterion

A compressed or mismatched sewn assembly can store elastic energy at equilibrium. Equilibrium requires balanced forces, not zero elastic energy. An autonomous, damped system must not sustain oscillation by repeatedly generating the energy it dissipates. Kinetic energy alone is insufficient: gravity and elastic strain can legitimately feed motion.

For fixed geometry, controls and material parameters, let

`E = K + U_membrane + U_bending + U_sewing + U_contact + U_gravity + U_grab`.

Use the actual mass-weighted kinetic energy in joules. Include gravitational potential, even though its zero level is arbitrary. For the current mass-proportional drag coefficient `c` (s⁻¹), continuous dynamics satisfy `dE/dt = P_control - c vᵀMv`. Adding physical internal damping would contribute another nonnegative loss. A moving grab, animated boundary or wind can supply work. A fixed pin does no work. A stationary spring stores energy but supplies no continuing external power.

This is different from VBD's **within-step variational objective**, which also contains inertia relative to a predicted target. Reducing that objective during local iterations does not establish decreasing mechanical energy from one accepted timestep to the next.

## A discrete balance that can be checked

The following is an independent algebraic decomposition for backward Euler, with the same potential `U` at both endpoints and `Δx = h v₁`:

`r = M(v₁-v₀)/h + cMv₁ + ∇U(x₁)`

`ΔE = -h c v₁ᵀMv₁ - ½ ΔvᵀMΔv + Q + rᵀΔx`

`Q = U(x₁)-U(x₀)-∇U(x₁)ᵀΔx`.

The first term is drag loss; the second is backward Euler's kinetic dissipation. `Q ≤ 0` for a convex potential. Cloth/contact energies are not globally convex, so that sign cannot be assumed. The residual is recomputed at the complete accepted configuration; per-vertex residuals sampled during successive local updates are not the final residual.

`rᵀΔx` is a diagnostic contribution, not an isolated proof of solver-generated energy: the residual can also include movement-bound reactions. Exact nonlinear backward Euler alone is not a universal mechanical-energy monotonicity guarantee for nonconvex cloth.

The contact feature set can change at detection. Evaluate the old and new contact models at the **same accepted position** and report their potential difference separately. Likewise, evaluate an old/new grab target at the same position to account for activation, motion and release of its spring. Dropping a grab removes its stored spring energy from the modeled system; it is not a cloth impulse.

## Implemented diagnostics

`src/vbd/audit.rs` provides explicit, read-only `audit_energy` and `audit_step` methods. They report kinetic, membrane, bending, sewing, gravity, contact and grab energies; drag, implicit loss, nonlinear remainder, residual work and an algebraic closure error. They reject inconsistent kinematics, invalid data, moving pins and non-finite terms. They do not change states, forces, prediction, damping, timesteps, iterations or contacts. No solver step or worker calls them automatically.

`examples/energy_probe.rs` runs a small 40-vertex two-panel fixture with 15% unequal sewn lengths: 120 settle steps, 60 smooth pulling steps, 30 hold steps and 150 release steps. The unchanged CPU VBD/OGC solver performs all simulation. A free-panel variant is also available. A separate cloned contact detector prevents auditing from altering the solver cache.

The same tool can replay an existing accepted-state trace entirely on CPU. It checks the initial state against the project, keeps its saved settings, freezes a diagnostic contact set across each interval, then records the contact-refresh energy difference. This evaluates an old GPU trajectory; it does not rerun GPU iterations or certify their internal contacts. CPU contact refresh inside a simulated step can also differ from the diagnostic frozen set. These distinctions remain explicit in the report.

```powershell
$env:CARGO_INCREMENTAL = '0'
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test energy_audit -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example energy_probe -- --output output/energy-audit
# Reuse an existing trace without using a GPU:
.\target\debug\examples\energy_probe.exe --project path/to/input.json --trace path/to/trajectory.jsonl --steps 360 --output output/replayed-energy
python scripts/research-seam-modes.py
```

Detailed results are recorded in [CPU evidence](energy-and-release-validation.json). Private garment geometry and trajectories remain ignored under `output/energy-research/`. Diagnostic runtime is not the application's step cost, and no GPU performance improvement is claimed.

### Observed results

| Case | Autonomous steps with measured energy rise | Interpretation |
|---|---:|---|
| Archived Garment, all 360 steps | 121 / 360 | Net +3.637 mJ; includes the restart transient |
| Archived Garment, excluding first 60 steps | 108 / 300 | Net **−19.463 mJ** despite local increases; no proof of indefinitely sustained motion |
| Unequal sewing, 16 CPU iterations, release | 50 / 150 | Maximum rise 0.375 µJ; no contact-refresh energy change in this fixture |
| Unequal sewing, 64 CPU iterations, release | 0 / 150 | Supports a convergence hypothesis, not a production fix or equal-cost comparison |
| Quadratic two-mass model, 16 scalar sweeps, release | 88 / 210 | Same fixed input/controls as the coupled case; peak rise 16.529 µJ |
| Quadratic two-mass model, exact coupled solve, release | 0 / 210 | Same energy law/drag; stored equilibrium energy remains 0.624688 mJ |

Rises count only values above 1e-12 J. The Garment audit closes algebraically within 1.43e-14 J, but that closure is not physical validation: extrapolating frozen contact features creates large mutually cancelling residual/remainder/refresh contributions. Do not attribute their individual totals directly to sewing, or claim that all apparent energy production comes from the solver residual. The fresh-potential aggregate and its limitations must be reported together.

The two cloth runs follow the same **relative** pull protocol but grab their own settled positions, so their absolute target paths differ. Their release displacements are respectively 33.62 mm and 26.56 mm toward the unpulled direction over the recorded interval. Both move after release; this is not a guarantee of indistinguishable animation. The free-panel protocol completes as well. The analytic free-fall unit test, rather than that long contact-bounded run, establishes the expected backward-Euler velocity law.

Three energy-accounting tests cover analytic free fall, deliberately injected acceleration, frozen contact energy and stationary prestress. Four existing VBD tests also pass. CPU-only all-target Clippy passes with warnings denied. The desktop executable and shader are unchanged in this pass.

## A different correction to investigate

The first candidate is **better coupled solving at stiff seams**, preserving the same constitutive laws and equilibrium. A seam quadrature sample couples several weighted vertices. Vertex-only updates can leave slow coupled modes unresolved under a finite iteration budget. A small block or a sparse coarse correction can address them together; it must include all incident material/contact terms, remain within each original OGC ball and use the unchanged local objective for acceptance. This is not a post-solve position projection or a replacement of the material rest state.

`scripts/research-seam-modes.py` tests that principle on two scalar masses with incompatible elastic support/seam rest lengths, during rest, pull, hold and release. It compares 16 scalar Gauss-Seidel sweeps with an exact coupled 2×2 backward-Euler solve. Mass, timestep, potential, drag and prescribed mouse path are identical. The positive equilibrium energy remains stored in both cases. This is an analytic quadratic model without contact, not an implemented cloth block solver.

If the audit instead points to contact-model changes, investigate force/feature consistency without shrinking conservative pair coverage or relaxing collision bounds. If it shows slowly decaying physical modes rather than numerical energy production, evaluate **objective strain-rate damping**. Forces must use all participating velocities and vanish for rigid translation/rotation of rotation-invariant strain measures. Damping world-space vertex velocities with a stiffness diagonal would brake free fall; it is not an acceptable shortcut. Even physical damping changes transient response and requires calibration and explicit validation.

For a rotation-invariant strain measure `q(x)`, a possible constitutive form is `f_d = -Jᵀ D J v`, where `J = ∂q/∂x` and `D` is positive semidefinite. Its instantaneous power is `-q̇ᵀDq̇ ≤ 0`, and rigid motion has `Jv = 0`. A nonzero sewing-gap **vector** rotates with a prestressed garment and is not such an invariant; choosing relative velocities alone only guarantees translation invariance. This needs careful scalar/metric strain choices, consistent units and derivatives. It remains research, and finite solve error can still defeat a discrete passivity claim.

A structure-preserving discrete-gradient integrator is a longer-term alternative for an explicit discrete energy law. It is a different integration project, not a cheap global energy clamp. No such backend is implemented here.

## Acceptance before touching the interactive backend

- Pull, hold and release the same material attachment with the same time-dependent target; account for its work.
- Compare total mechanical energy, kinetic decay, seam stress and stretch, finite forces, contact bounds and transient trajectories. Include an untouched prestressed equilibrium and rigid free motion.
- Check that release starts moving immediately and relaxes smoothly; low RMS achieved by sticking fails acceptance.
- Recheck the owner's Garment with actual GPU interaction when authorized. The current CPU fixture is not a reproduction of that precise interaction.
- Measure full-step mean/P95 and delivered steps/s after any GPU implementation. Keep the 60 steps/s target and disclose tails; no timing claim follows from this CPU study.

## Primary sources consulted

- Baraff and Witkin, [Large Steps in Cloth Simulation](https://www.cs.cmu.edu/~baraff/papers/sig98.pdf), §4.5: damping through deformation rates rather than the derivative of elastic energy alone. The law's power is nonpositive when formed with nonnegative coefficients.
- Anka He Chen, [Implementing VBD Damping Properly](https://ankachan.github.io/posts/2026/03/implementing-vbd-properly/), March 2026: full relative-velocity force stencils, diagonal local solve blocks and rigid-motion invariance. No damping law from this note is added to ClothCraft.
- Anka He Chen, [A Sparse Coarse Space for VBD from Element Eigenmodes](https://ankachan.github.io/posts/2026/05/vbd-sparse-coarse-space/), May 2026: a proposed sparse correction for coupled modes. This is an author's research note, not a demonstrated ClothCraft speedup or an available backend.
- McLachlan and Quispel, [Discrete gradient methods have an energy conservation law](https://arxiv.org/abs/1302.4513): abstract consulted as a pointer for structure-preserving integration; no implementation claim.
