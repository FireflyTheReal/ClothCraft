# Smooth forest averaging: CPU contact experiment

`src/ogc/smooth_forest.rs` now implements this derivation as an isolated CPU
kernel, not a solver, a validated cloth contact model, or a claim of novelty.
The [hard forest experiment](wire-quadrature-audit.md)
preserves a barrier lower bound but fails force continuity. Its 222.5 N cusp
must remain a regression gate.

## Candidate construction

For one source edge, create a graph whose nodes are nearby target edges.
Node `i` has finite-primitive barrier `a_i`. Two target edges meeting at a
target vertex define a graph link with barrier `c_ij` from that vertex to the
source edge. Use the complete clique at each shared vertex, not an arbitrary
ID-dependent tree. Distances imply `0 <= c_ij <= min(a_i, a_j)`.

For any graph forest `F`, define
`U_F = sum_i a_i - sum_links_in_F c_ij`.
Root each tree at its largest `a_i`. Each nonroot contribution
`a_i - c_parent(i)` is nonnegative, hence `U_F >= max_i a_i`.
A convex average over forests preserves this bound.

The proposed smooth distribution uses positive link weights
`w_ij = c_ij / tau`, where `tau > 0` has units of joules. With the weighted
graph Laplacian `L`, let `M = I + L`.
[Chebotarev and Shamis, Matrix-Forest Theorems](https://arxiv.org/html/math/0602575v2)
relate `det(M)` to the total weight of rooted spanning forests. The forest
weight is the product of its link weights; root multiplicities are included.
This theorem supplies a way to average over all forests without enumeration.

Differentiating the partition function suggests link inclusion probabilities
`p_ij = w_ij * (M_inverse[ii] + M_inverse[jj] - 2*M_inverse[ij])`.
The candidate energy is then
`U = sum_i a_i - sum_links c_ij * p_ij`.
The probability formula and energy agree with exhaustive enumeration on the
tested tiny graphs, including disconnected positive-weight components.
`tests/smooth_forest.rs` differentiates the full matrix computation.

All probabilities depend on geometry. Differentiating only `a_i` and `c_ij`
while freezing `p_ij` would be a different, inconsistent force law.

The implementation uses a Helmert basis orthogonal to the constant mode of
`L`, builds the reduced matrix directly from incidence outer products, and
performs differentiated Cholesky solves. It does not clamp probabilities or
shift pivots. Invalid inputs, nonfinite results or failed positive pivots
return errors. Structural connected components are now factored independently;
each dense component is limited to 128 nodes. Zero-weight links remain in
their component because their derivatives may be nonzero.

For a structural component of exactly two incident target edges, the algebraically equivalent form
`(A+B-c) + c*tau/(tau+2*c)` uses the existing patch kernel to cancel the common
feature **before** differentiation. The naive expression selects incompatible
one-sided Hessians at an exact shared endpoint: the planar test found an extra
`-225000 N/m` of tangential curvature. The reduced form removes it and is checked
against the analytic expression and finite differences. This does not establish
correct second derivatives at all higher-valence feature ties.

The subsequent [boundary and factorization pass](forest-factor-and-boundary.md)
adds symmetric generalized Hessians at an exact single-endpoint/interior
boundary. This is needed when an inactive extension keeps a two-edge patch
inside a larger connected graph. The selection changes neither finite-region
values nor first derivatives. It is not a smooth-distance replacement and
does not resolve double-endpoint or parallel degeneracies.

As `tau` tends to zero, the distribution favors forests with the largest
number of positive-weight links, with product-weight probabilities among them.
It does **not** generally become the earlier hard maximum-score forest on a
cyclic graph. On a two-edge junction it approaches the exact patch subtraction.

## Useful properties to verify

- The forest-average interpretation gives a pointwise barrier lower bound
  in exact arithmetic. It does not prove an accurate cloth response.
- Zero-barrier graph links have zero weight. Adding an inactive isolated
  node adds an identity block to `M`, leaving the energy unchanged.
  This could avoid refresh jumps when used with a conservative force halo.
- At a two-node junction the expected subtraction is
  `2*c*c / (tau + 2*c)`. The missing subtraction relative to the exact
  patch is `c*tau / (tau + 2*c)`. This introduces a tunable sliding error;
  it must be measured, not described as frictionless.
- Smooth graph weights can remove hard merge-order choices. They do not
  automatically smooth the finite segment distance at parallel degeneracies.

## Required falsification gates

1. Compare value and inclusion probabilities against explicit rooted-forest
   enumeration, including zero weights, disconnected components and ties.
2. Differentiate through the complete matrix solve; check gradients and
   Hessians, objectivity, net force/torque, and the barrier lower bound.
3. Revisit the measured hard-forest cusp, exact feature ties, the planar
   slide, and matched pull/hold/release without damping.
4. Sweep `tau` and graph size. Small `tau` reduces sliding bias but can make
   `I+L` poorly conditioned. Reject any apparent fix caused by numerical
   probability clipping or hidden regularization.
5. Resolve near-parallel EE contact and add a compatible vertex–face
   discretization. This proposal alone is not a surface contact model.
6. Test on the frozen dress with complete candidate halos, independent
   collision audits and full target-objective convergence before cost tests.

The published [Geometric Contact Potential](https://huangzizhou.github.io/research/smooth-contact.html)
remains a separate route with explicit shape-differentiability goals.
No GCP implementation or smooth forest **backend** exists in this pass.

## Reproduce the kernel gates

Use two CPU threads. The probe writes only a fresh output path; it never saves
over a project or launches the editor.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test smooth_forest --example contact_patch_probe -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example smooth_forest_probe -- --output output/smooth-new/static.json
cargo run --locked --no-default-features -j 2 --example contact_patch_probe -- --output output/smooth-new/pull.json --smooth-tau 1e-3,1e-5,1e-7,1e-9,1e-11
python scripts/summarize-smooth-forest.py output/smooth-new/static.json
python scripts/summarize-contact-patch.py output/smooth-new/pull.json
```

`smooth_forest_probe --project <portable-project.json>` additionally samples
the worst signed densities and the largest active target graphs at 3/6/12 mm.
`--state <state.json>` uses an external snapshot while retaining the project's
original world initialization. This is selected graph evaluation, **not cloth
stepping or a complete dress contact audit**. Oversized graphs remain explicit
failures. See [results and remaining gates](smooth-forest-contact-results.md).
