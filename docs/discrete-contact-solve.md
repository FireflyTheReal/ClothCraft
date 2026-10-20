# Coupled discrete contact solve: synthetic pilot

Follow-up: [rank-aware search and corrected physical convergence checks](contact-rank.md)
now pass one complete protocol; three still fail. The results below describe
the earlier pilot and remain useful negative controls.

This is an opt-in CPU experiment in `deforming_contact_probe`, not an editor
backend or a fix validated on the saved dress. It advances time and recomputes
reactions; it goes beyond the preceding [offline work audit](discrete-contact-work.md).

## Equations and search

For one backward-Euler step, solve material/inertia/grab force balance against
nonnegative contact reactions, with endpoint gaps nonnegative and complementary.
The invariant discrete distance gradient supplies the **force** row. The ordinary
endpoint distance derivative supplies the **geometric** row. These are generally
different. Material rest, material law, masses and timestep are unchanged.

`--discrete-forces` uses a symmetric geometric contact QP as a search model.
At each iteration it subtracts the lagged difference between discrete and
endpoint reactions from the search gradient. Physical convergence is checked
against the current discrete reactions, not the search QP's reactions. Every
multiplier starts at zero at the beginning of a physical step. There is no
inter-step force warm start, position clamp, added damping or sleeping.

`--newton-forces` also proposes a nonsymmetric coupled Newton direction:

```
[ D_x(material/inertia force - discrete reaction)   -F ] [dx] = [-force residual]
[ G                                                 0 ] [dl]   [-gap           ]
```

Here F contains discrete force rows and G contains endpoint geometric rows.
The small-fixture oracle differentiates the force residual with central position
differences (1e-7 m at this scene scale). Multiplier derivatives are linear.
Positive physical-QP reactions select this trial active set. Search-path repair
rows never become physical reactions. Singular Newton matrices and negative
proposed multipliers are explicitly rejected; the geometric QP proposal remains.
No diagonal regularization is applied. This active-set policy is not yet robust.

Both proposals pass the original OGC movement bounds and every existing swept
VF/EE certificate. A residual/complementarity merit chooses a decreasing safe
trial. Its scales are 1e-7 N and 1e-10 m, not compliance or physical damping.
Each candidate gets at most 32 backtracks. The better accepted merit is retained.
An unsuccessful search stops explicitly. The energy audit is observational:
energy is never rescaled or used to conceal an unresolved force balance.

## Results and limits

Detailed numbers and failures are in [the validation record](discrete-contact-solve-validation.json).
The folded 16-vertex fixture has three panels, one seam and 266 eligible VF/EE
pairs. Matching and 10%-eased seams share the same algorithm and settings.

Two previously energy-increasing **individual post-release steps** now converge
with decreasing mechanical energy and the unchanged collision gates:

| Case | Previous energy change | New energy change |
| --- | ---: | ---: |
| Matching seam, step 176 | +1.08536e-6 J | -1.53257e-7 J |
| 10% eased seam, step 163 | +1.42535e-8 J | -2.21077e-8 J |

These are matched incoming states, not full corrected trajectories. The new
force-balance residuals are below 2e-9 N. The energy decomposition closes below
1.1e-20 J. Eased contact work is slightly positive (6.3e-14 J), within the finite
complementarity tolerance; no exact nonpositive-work claim is made.

**The complete protocol fails.** From initial placement, the eased case stalls
at step 11 with redundant contact rows; the Newton proposal reports rank
deficiency. From the saved pre-grab state, eased moving release stops during
pulling at step 64; matching held-release stops at step 62 on a failed physical
QP. Neither reaches its requested release. Separate replays beginning exactly at
release complete three steps each, then fail at steps 103 (matching moving) and
133 (eased held). The unsewn eight-vertex control also stalls at step 13 before
the grab, so the remaining failure is not specific to unequal seams.
These failures are retained, never
reported as settled cloth. No complete pulling/releasing or fluidity gate passes
on the sewn fixture. No GPU or new whole-dress replay was performed.

The lagged-only method and the first Newton-only search are negative controls
in `output/discrete-solve-20261020/`: each fixes one isolated peak and fails the
other or the full trajectory. The final implementation compares both proposals.
These controls were run before that final search policy, and their filenames
must not be confused with final `hybrid-*` results.

This dense finite-difference oracle is deliberately expensive. Isolated step
costs include diagnostics and are not realtime benchmarks. It does not establish
that production jitter has been fixed, nor that the production energy jump has
the same cause. The editor, VBD/GPU path and user garment remain unchanged. Replaying the two
endpoint baseline steps with the final binary reproduces the previous positions
and velocities exactly. Twenty-five targeted CPU tests, all-target CPU Clippy,
formatting and diff checks pass. This does not override the trajectory failures.

## Next gates

1. Handle redundant active geometry and reaction nonuniqueness in the coupled
   equations, checking the full residual rather than dropping collision rows.
2. Globalize active-set changes while keeping the complete swept constraints;
   prevent tiny accepted search steps from masquerading as useful convergence.
3. Pass all matching/eased, moving/held sequences from initial placement and from
   checkpoints. Retain work, force, sliding and geometric gates independently.
4. Validate force derivatives and timestep convergence, then replace the dense
   finite-difference search with a scalable formulation. Only then replay the
   saved dress and compare full-step performance.

## Reproduction

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo build --locked --no-default-features -j 2 --example deforming_contact_probe
cargo test --locked --no-default-features -j 2 --test contact_newton --test contact_residual --test discrete_contact --test dense_contact --test deforming_contact --example deforming_contact_probe -- --test-threads=2
```

The final research flags are `--discrete-forces --newton-forces`. Keep the
existing `--independent-block --pivot-block --swept-repair --accumulate-repair
--iterations 384 --audit-energy --audit-iterations --stop-on-unconverged`.
Use `--sewn-ease 0` or `0.1`; `--held` changes the release protocol. Omitting both
new flags retains the endpoint baseline. Reproduction commands, source hashes,
binary hash and captured failures accompany the local evidence directory.
