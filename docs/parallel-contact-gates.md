# Parallel contact: rejected weighting and feature-transition kernels

20 October 2026. CPU research only. Production VBD/OGC, its collision bounds,
GPU code, rest configuration, prediction and damping are unchanged.

## A positive energy is not enough

The earlier wire audit showed that mollifying positive edge barriers while
leaving common-feature subtraction unchanged can make the energy negative.
An independent trial now weights both terms: node barriers are `m_i*a_i`,
and a link uses `m_i*m_j*c_ij`, with `0 <= m_i <= 1`. This preserves the
forest bound because each link remains below both of its node barriers.

It fails a stronger test. For two collinear targets at their shared point,
`a_1=a_2=c=b`. With equal weights `m`, the energy tends to
`(2m-m²)*b` as forest `tau` tends to zero, instead of the single representative
`m*b`. At `m=0.5` this is 50% excess normal force. Decreasing `tau` cannot
remove it. This is a duplicate-consistency comparison within the weighted
wire-only experiment, not a comparison to the installed VBD force law.
The test deliberately preserves this failure as a rejection gate.

The angle sweep gives another example: at 0.5 degrees, the product-link model
pushes with 61.10 N against 32.96 N for the single representative. The old
positive-only signed sum is negative in that configuration. Neither trial is
installed.

There is a structural tradeoff in this particular family of scalar link
weights. A differentiable function `f(x,y) <= min(x,y)` cannot also satisfy
`f(t,t)=t` at an interior point: `f-x` and `f-y` would both attain a local
maximum there, requiring simultaneously `grad f=(1,0)` and `(0,1)`.
This does not rule out other contact formulations. It rules out obtaining
smoothness, that pointwise bound and exact duplicate cancellation merely by
choosing a better scalar smooth-min link weight.

## Implemented literature component

[Geometric Contact Potential, §5.3 and Appendix A](https://arxiv.org/html/2402.00719v3)
were read. Its feature mollifiers suppress higher-dimensional terms as closest
points reach a boundary; lower-dimensional contacts provide the handoff.
The complete formulation also includes directional factors and integration
weights.

`src/ogc/feature_mollifier.rs` independently implements only Appendix A's
dimensionless vertex–edge, vertex–face and edge–edge factors. Their input
ratios compare distances to the primitive and its boundary features.
The example width is 0.01. All participating positions are differentiated.
These are **not a force law, a GCP backend, or collision safeguards**.

The factors are C1; no global C2 claim is made. At exact support boundaries
the code selects the zero-side Hessian. Intersections, nonfinite distances,
nonpositive widths and degenerate target primitives return errors.
Degeneracy thresholds match the distance helpers: squared edge length
`1e-24 m²` and squared cross-product norm `1e-30 m⁴`.

## Evidence

Five new CPU tests cover support, endpoint and parallel limits, finite
differences of gradients/Hessians for all participating vertices, force/torque
balance, rigid-motion and uniform-scale invariance, and the rejected
product-weight model. The older smooth-forest, primitive-contact, patch and
moving/held-release tests are also run. Passing these does not validate a
trajectory under a new mollified force law; none is assembled yet.

At exactly parallel edges the new EE factor is zero. In the shared-vertex
fixture the VE factor remains one. This illustrates why multiplying existing
EE terms alone would discard the required lower-feature contribution.

A fresh CPU detector on the frozen dress at 49.45 s has 694 positive-energy
production terms. Applying only the diagnostic factors gives:

| Existing term | Zero factor | Fractional factor | Unit factor | Errors |
| --- | ---: | ---: | ---: | ---: |
| Edge–edge | 0 | 17 | 450 | 0 |
| Vertex–edge | 0 | 0 | 2 | 0 |
| Vertex–face | 0 | 24 | 201 | 0 |

This is a snapshot classification of existing terms. It is not the complete
candidate set needed by GCP, does not establish lower-feature coverage and
does not identify the cause of the dress's jitter. No dress stepping, GPU
workload, new collision guarantee or FPS measurement is claimed.

## Next implementation boundary

A complete candidate must assemble VF, EE, VE and VV contributions together,
with explicit ownership and seam exclusions; include all geometry used by
directional factors in differentiation and coloring; and handle open,
two-sided cloth. Quadrature units and reference geometry need an explicit
contract. Preserve the original OGC geometric guards during comparison.
Then require refresh continuity, planar sliding, full-model residuals and
matched dress pull/release before performance comparisons.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test feature_mollifier -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example parallel_mollifier_probe -- --output output/parallel-new/probe.json
# Optional read-only frozen-dress classification:
# add --project <portable-project.json>
```

Raw evidence: `output/parallel-mollifier-20261020`.
See [validation](parallel-contact-validation.json) for hashes and checks.
The pass has 25 successful targeted CPU tests; CPU/desktop all-target Clippy,
formatting and tracked-diff whitespace checks pass.
