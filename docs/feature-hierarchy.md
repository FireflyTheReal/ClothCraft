# Contact feature handoff audit

This CPU research module checks which geometric pairs are available when a
higher-dimensional feature weight vanishes. It does not add forces, change
the production VBD/OGC law, advance cloth or install a new solver.

## Why this is needed

The previous parallel-edge experiment cannot safely be completed by simply
multiplying the existing forest terms by weights: that construction failed
the duplicate-force gate. See [the measured rejection](parallel-contact-gates.md).
An alternative contact formulation needs lower-dimensional features to take
over when an edge–edge or vertex–face contribution vanishes.

The [GCP paper](https://arxiv.org/html/2402.00719v3) describes VF, EE, VE and
VV terms with feature mollification and directional factors. Its exterior
criterion is derived for oriented volume boundaries (§4, §5.2.1 and Appendix
B). Applying that criterion to open, two-sided cloth requires an explicit
adaptation; the Appendix A kernels alone are not a cloth contact model.

## Implemented contract

`ogc::feature_hierarchy` queries four families directly against fresh BVHs:
vertex–face, unordered edge–edge, vertex–edge and unordered vertex–vertex.
Each pair must involve a moving vertex. Shared vertices and the world's
explicit cross-seam exclusions are rejected. Lower features are queried
independently: an excluded face must not hide an otherwise eligible edge.

Finite primitive distances filter the broadphase results at the requested
radius. Broadphase padding covers rounding at the query boundary; it does
not enlarge the final distance threshold. Results are sorted and deduplicated,
with duplicate visits reported. There is no pair-count truncation.

The coverage audit visits active lower features recursively. It reports
missing dynamic pairs, zero-weight parents without a positive lower witness
and factor errors, including coincident vertices. A wholly fixed child of a
mixed fixed/moving parent can be a geometric witness without participating
in the dynamic pair list; these visits are counted separately.

Coverage is valid only for the exact positions and feature topology used
to construct the snapshot. Evaluation after movement or a topology change
is rejected. This is not a temporal cache or a conservative force halo.

The hierarchy deliberately includes nearby eligible mesh neighbors. Applying
bare barriers to all these pairs would introduce unwanted forces. Directional
selection, two-sided conventions, quadrature, consistent differentiation
and solver incidence must be implemented and tested before force assembly.
A positive lower-feature witness proves neither a minimum repulsive force
nor an intersection-free trajectory.

## Verification

The independent small-mesh oracle uses closest-point constructions and
exhaustive enumeration. Tests cover all four families, partial fixed/moving
layouts, seam exclusions, face boundaries, parallel edges, coincident and
degenerate inputs, stale snapshots and unchanged production contacts/bounds.
Existing derivative, patch, forest and moving/held-release tests remain gates.

The headless probe loads a portable project and optionally a previously
saved state. It preserves the original checkpoint references, audits only
snapshots, and compares production contacts and geometric bounds before and
after. Its wall times are audit costs, **not simulation FPS**.

```powershell
$env:RAYON_NUM_THREADS = '2'
cargo test --locked --no-default-features -j 2 --test feature_hierarchy -- --test-threads=2
cargo run --locked --no-default-features -j 2 --example feature_hierarchy_probe -- `
  --project <portable-project.json> --output output/hierarchy-new/dress.json
# Add --state <saved-state.json> to inspect an existing pull/release endpoint.
```

Full directional coverage, contact refresh continuity, frictionless sliding
and a new matched dress pull/release trajectory remain required before
promotion. The installed editor is unchanged.

## Frozen-dress observations — 20 October

The source checkpoint is the existing portable dress at 49.45 s. The second
snapshot is the prior production pull/release endpoint at 50.1167 s, loaded
with the original checkpoint references. **No new dress trajectory was run.**

| Snapshot, activation radius | Active VF / EE / VE / VV | Zero-factor VF / EE / VE / VV |
| --- | --- | --- |
| Dress, 3 mm | 233 / 492 / 8 / 0 | 8 / 25 / 0 / 0 |
| Release, 3 mm | 228 / 437 / 4 / 0 | 2 / 19 / 0 / 0 |
| Dress, 6 mm | 6777 / 20850 / 4877 / 220 | 4415 / 12572 / 1293 / 0 |
| Release, 6 mm | 6450 / 19795 / 4605 / 203 | 4152 / 11915 / 1196 / 0 |
| Dress, 12 mm | 23696 / 66892 / 23714 / 1678 | 20194 / 54536 / 9640 / 0 |
| Release, 12 mm | 22229 / 62635 / 22119 / 1557 | 18869 / 50919 / 8985 / 0 |

All six audits report zero missing active lower pairs, uncovered zero-factor
parents, factor errors and duplicate query visits. Production contacts,
centers, radii and directional bounds compare equal before and after each
snapshot audit.

At 3 mm activation, fresh hierarchy construction (6 mm query) took about
160 ms per snapshot in the optimized development build on the local CPU;
coverage took 3–5 ms. These are single observations including new BVH
construction, not a throughput benchmark. This standalone implementation
does not fit a 16.7 ms step budget and is not on the runtime path.

The pass has 31 successful CPU tests: six hierarchy tests, five mollifier
tests, five patch tests, four finite-primitive tests, nine forest tests and
two isolated moving/held-release tests. The last two exercise the existing
patch-law experiment, not an assembled four-family potential.

Raw evidence is in `output/feature-hierarchy-20261020`; reproducibility
metadata and hashes are in [the validation record](feature-hierarchy-validation.json).
