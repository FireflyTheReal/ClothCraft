# Contact patch quadrature experiment

Status: isolated CPU kernels and headless tests. **Not a new backend, an editor
option, or a validated dress fix.** Production VBD/OGC, GPU packing, candidate
search, collision bounds, damping and prediction are unchanged.

## Why test this

The [finite-distance experiment](primitive-contact-results.md) removed measured
refresh jumps but created tangential forces on a flat triangulated surface.
The next gate must reject both energy discontinuities and artificial sliding
resistance. Decreasing energy alone is not a sufficient acceptance condition.

[Convergent IPC, sections 4.2–4.4](https://arxiv.org/html/2307.15908)
constructs a geometric approximation to a maximum barrier using primitive sums
and common-feature subtraction. Its whole-mesh positivity argument requires a
sufficiently small activation neighborhood. Its complete discretization also
contains quadrature weights and edge terms. This experiment implements only
two local instances, using ClothCraft's existing activation function. It is
neither a reproduction of that complete method nor GCP.

## Implemented kernels

`src/ogc/patch_contact.rs` evaluates:

- Point against two triangles sharing an edge:
  `U = b(d(point, face A)) + b(d(point, face B)) - b(d(point, shared edge))`.
- Segment against two segments sharing an endpoint:
  `U = b(d(source, edge A)) + b(d(source, edge B)) - b(d(source, shared point))`.

All five vertices participate in automatic derivatives. There are no frozen
barycentric coordinates, weights, normals, or garment identifiers.

For each two-element patch, the common feature is contained in both elements.
Since the activation decreases with distance, each positive term is at least
the subtracted term. Thus `A+B-C >= max(A,B) >= 0` for separated geometry.
The kernel does not clamp a negative energy to conceal an invalid model.

Closest-feature cancellation is performed before automatic differentiation.
At exact feature ties, separately differentiating the three terms could select
incompatible one-sided Hessians and create tangential curvature on a flat
patch. The simplified expression removes that artifact. Feature decisions use
the geometry, without an energy threshold or damping rule. No global C2
regularity is claimed.

These kernels do not provide a general surface assembly. Multiple incident
faces, boundaries, exclusions, reciprocal contributions, rest quadrature and
global positivity still need treatment. The wire kernel is not mollified near
parallel edges. Coincident primitives are outside its domain; shared
coincidence can produce infinity minus infinity. Do not install these kernels
in the production detector by summing overlapping patches.

## Validation protocol

`tests/patch_contact.rs` checks flat tangential force, exact-tie tangential
curvature, all-coordinate gradients/Hessians, translation/rotation invariance,
net force/torque, continuity, and the two-element positivity bound. The folded
positivity fixtures remain strictly separated; the initial fixture sweep
included actual intersections and was corrected rather than treating infinite
barriers as finite results.

`examples/contact_patch_probe.rs` compares an analytical plane, naive finite
primitive sums, and the two corrected kernels. One point or a rigidly
translating transverse segment moves over a fixed planar patch. It uses a
backward-Euler objective, shifted Newton directions and the existing OGC step
truncation. This is a diagnostic integrator, **not `VbdSolver::step`**.
The plane bounds protect the whole translating segment because its endpoints
share the probe's height. This does not test deforming segment collisions.
The naive-sum controls represent the preceding rejected experiment, not the
installed production contact law.

The protocol uses 480 steps at 1/60 s, a 20 g probe, and a constant 225 N external
normal preload producing a 1 mm equilibrium gap. The preload is a stress test,
not fabric weight. A 5000 N/m handle moves 8 mm, then releases either in motion
or after a one-second hold. No damping, friction, sleeping, or velocity
correction is applied. The analytical moving-release reference retains
4 mm/s tangential velocity.

Residuals are measured at accepted endpoints. Unfinished solves are reported,
not omitted; the tolerance is 1e-6 N, with at most 64 iterations. The first
1e-7 N trial ended at 1.003e-7 N after its iteration limit; the relaxed stopping
tolerance is recorded explicitly. Naive sums can
fail the final tolerance and can leave the finite test patch; such runs stop
and explicitly record the boundary exit. Mechanical-energy comparisons start
after release and exclude the change in handle potential.

## Reproduction

Choose a fresh output path; traces are never overwritten by the runner.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test patch_contact -- --test-threads=2
cargo test --locked --no-default-features -j 2 --example contact_patch_probe -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example contact_patch_probe -- --output output/contact-patch-new/trace.json
python scripts/summarize-contact-patch.py output/contact-patch-new/trace.json --plot output/contact-patch-new/trace.png
```

The snapshot under `output/patch-contact-20261020` is local ignored evidence.
No GPU, desktop window, executable replacement, or user document modification
is needed. Results from this small diagnostic cannot establish dress jitter,
production contact-refresh continuity, or frame rate.

## Results on 20 October

Local evidence: `output/patch-contact-20261020/pull-release-final.json`,
its `.summary.json`, and `pull-release-final.png`. The earlier `pull-release.json`
records the pre-cancellation evaluation for comparison.

| Quantity | Face-pair correction | Wire-pair correction |
| --- | ---: | ---: |
| Accepted steps, both release protocols | 960 | 960 |
| Maximum position error against analytical reference | 0 m | 1.36e-20 m |
| Maximum velocity error against analytical reference | 0 m/s | 8.67e-19 m/s |
| Minimum separation | 1 mm | 1 mm |
| Unfinished solves | 0 | 0 |
| Maximum measured post-release energy increase | 0 J | 0 J |

These very small errors are expected for an algebraically equivalent flat
patch, not evidence of accuracy on general cloth. The moving-release speed
remains 4 mm/s; held-release ends at rest. The largest residual across the
corrected runs is 7.94e-7 N, below the declared 1e-6 N gate.

The naive face sum deflects the probe and has one unfinished solve, with
2.76e-3 N residual at release. Both naive wire runs reach the finite fixture's
boundary early (398 and 458 steps). Their post-release energy is nonincreasing
up to roundoff, yet their motion is wrong. A monotone energy monitor therefore
cannot certify a valid contact discretization.

There is no dress trajectory or speedup measurement in this experiment.

## Next integration gates

1. Specify complete patch ownership and signed quadrature, including an
   activation-scale validity check and exclusions at seams.
2. Handle nearly parallel EE contacts consistently with the subtracted terms;
   multiplying only the positive EE terms can destroy the positivity bound.
3. Replay the captured EE and PF refresh transitions with full assembly.
4. Test folded cloth and the frozen dress, including matched pull/hold/release
   and independent collision audits, before measuring production cost.
