# Contact graph factorization and endpoint derivatives

CPU research follow-up, 20 October 2026. This changes only the experimental
smooth-forest kernel and its headless probes. The installed VBD/OGC model,
collision bounds, GPU code, damping and prediction remain unchanged.

## Why the previous small test was insufficient

The two-edge straight-line fixture passed because its combined energy was
simplified before automatic differentiation. Adding an inactive, disconnected
edge disabled that simplification in the old kernel. Adding an inactive but
connected extension still disabled it after structural factorization.

`forest_factor_probe` records the latter failure: at `tau=1e-7 J`, the central
difference of the tangential gradient is about `-1.34e-8 N/m`, but AD reported
`-225000 N/m`. The force and value were continuous. This is an incorrect
curvature for a locally smooth combined energy, not an energy jump.

## Independent graph components

The rooted-forest partition matrix is block diagonal across structural
components. Its determinant factors and each link's effective resistance
depends only on its own block. The energy and all derivatives therefore split
exactly. The implementation maps each component to local indices and restores
probabilities to the original link order.

This is a topology-only split. **Zero-weight links are retained**, including
their derivatives. Removing them just because their value is zero would be
incorrect for the generic AD kernel. The dense limit is now 128 nodes per
component; large disconnected graphs can be evaluated without dropping nodes.
A 300-node regression checks values, gradients and Hessians against independent
two-node formulas. A connected 129-node graph of zero-valued variable links
still returns an explicit error.

Two-node algebraic common-feature cancellation is now applied per component.
Disconnected padding and permutations preserve the contact value, force and
exact-tie curvature.

## Single endpoint/interior boundary

A finite segment distance is generally C1, not C2, where its closest point
reaches an endpoint. Both limiting branches have the same distance and
gradient but different Hessians. Choosing endpoint branches independently can
spoil a cancellation in a combined energy that is itself smooth.

The smooth-forest geometry path now averages those two limiting distance
branches **only at an exact single-endpoint/interior boundary**. Composition
with the barrier selects their symmetric generalized Hessian. This matches
central differences of the gradient on the tested boundary. No interval is
blended, no feature coordinate is frozen, and no damping or positional
projection is added.

The boundary predicate tests endpoint stationarity directly: project the
endpoint onto the opposite supporting line, require an interior projection,
and test orthogonality of its residual to the endpoint's incident segment.
Comparing an unconstrained line parameter with exactly one had introduced
endpoint-reversal asymmetry through denominator roundoff. No distance
threshold was added to hide that failure.

The original finite-distance function is unchanged. Other experimental laws
and production contact continue using their original paths. Double-endpoint
ties, near-parallel degeneracies, floating-point ambiguity at general rotated
ties, and a compatible vertex–face assembly remain open gates. A generalized
Hessian at a nonsmooth boundary is not a claim of global C2 regularity.

## Reproduction and scope

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test smooth_forest --test primitive_contact --test patch_contact --example contact_patch_probe -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example forest_factor_probe -- --output output/factor-new/probe.json
cargo run --locked --no-default-features -j 2 --example contact_patch_probe -- --output output/factor-new/pull.json --smooth-tau 1e-7,1e-9 --extend-wire
```

The microbenchmark compares the same scalar law and complete AD outputs with
and without component factorization, using `black_box`, seven samples per
ordering and both execution orders. Connected graphs with explicit zero links
are controls. Timings concern isolated CPU kernels, not full cloth steps or GPU
FPS. The `evaluate_monolithic` API is a research control, not another solver.

The extended moving/held-release probe keeps the same physical protocol and
adds a connected inactive target edge. It must preserve the trajectory and
separation; a lower rest-motion metric alone is insufficient.

Local evidence is in `output/forest-factor-20261020`; measurements and validation
are recorded in [the machine-readable record](forest-factor-validation.json).

## Measured results

The connected-extension curvature is now `-1.36497e-8 N/m`, against
`-1.34441e-8 N/m` from central gradient differences. The original pair and
disconnected-padding variants both give `-1.35523e-8 N/m`. Their energy is
identical at the reported precision and their normal force is approximately
225 N. Endpoint-reversal tests and every participating vertex's derivative
block pass. The boundary derivative test uses a `1e-10 m` finite-difference
step: the initial `1e-9 m` step had first-order truncation error at a C1 boundary;
the derivative tolerance was not relaxed.

Eight smooth-model trajectories (two tau values, with/without extension,
moving/held release) complete 480 steps each, with no unfinished solves or
measured post-release energy rise. Minimum separation remains approximately
1 mm. Against the analytical plane, maximum position/velocity errors are
`9.64e-10 m / 5.79e-8 m/s` at `tau=1e-7 J`, and
`7.77e-13 m / 4.62e-11 m/s` at `tau=1e-9 J`. The artificial sliding bias measured
by the earlier adaptive static scan still exists; these endpoint errors do not
establish exact frictionlessness.

The two frozen dress snapshots retain the same coverage as the previous pass:
1,620 selected graph evaluations satisfy the energy lower bound, while 75
12 mm stress evaluations still exceed the component limit. The checkpoint's
largest energy change relative to the previous kernel is `9.95e-14 J`.
No new cloth trajectory was simulated.

Illustrative medians from the first microbenchmark order, AMD Ryzen 7 2700X,
single thread, Rust dev profile with `opt-level=1`:

| Scalar / graph | Factored (µs) | Unfactored (µs) |
| --- | ---: | ---: |
| f64, 8 nodes in independent pairs | 5.0 | 3.0 |
| f64, 32 nodes in independent pairs | 20.4 | 84.2 |
| Jet3, 32 nodes in independent pairs | 36.1 | 1477.0 |
| Jet3, 96 nodes in independent pairs | 84.7 | 35283.0 |
| Jet3, connected 96-node control | 63475.5 | 60172.1 |

The reverse execution order is archived too. Timings vary; tiny scalar graphs
pay partition overhead and connected graphs have no asymptotic benefit.
Other workstation load and CPU clocks were not controlled. These numbers
support component factorization for disconnected graphs, not a general speedup
or a realtime claim.
