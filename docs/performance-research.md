# Performance research and selected implementation — 2026-10-14

The current bottleneck is CPU candidate construction, not the local compute shader. Instrumentation now separates BVH refit, candidate construction and exact narrow phase in `VbdMetrics.contact_profile`. These CPU wall times exclude graph coloring; the hybrid GPU backend's `detection_ms` includes it (the CPU backend times coloring separately from detection). GPU hardware timestamps remain separate.

Retained implementation, paired measurements and failures: [validation report](perf-fabrics-validation.md). The local apron comparison observes a 26.68% full-step cost reduction with equal final states; only the tested T-shirt window meets the realtime threshold.

## Sources actually consulted

- [Vertex Block Descent, 2024](https://graphics.cs.utah.edu/research/projects/vbd/vbd-siggraph2024.pdf): the paper describes cooperative GPU local assembly and difficulties with stiffness ratios. Our cooperative solve already exists; replacing the physical warm start with slower artificial motion is not an optimization.
- [Offset Geometric Contact, 2025](https://graphics.cs.utah.edu/research/projects/ogc/): revisited the project and paper's performance/convergence discussion. Published fully GPU examples are not a benchmark for our CPU-detected implementation. Keep reciprocal VF, EE and the original detection balls.
- [Augmented Vertex Block Descent, 2025](https://graphics.cs.utah.edu/research/projects/avbd/): read the official overview describing augmented constraints and convergence under disparate stiffness. The large PDF fetch failed in this pass. AVBD is a promising separate solver/convergence task, **not implemented** by this change.
- [I-Cloth, 2018](https://min-tang.github.io/ICloth/): read the official overview of temporal/spatial coherence and GPU incremental collision handling. Our candidate cache only reuses pair IDs with a conservative envelope; it is not their CCD/impact-zone algorithm.
- Recent material work and available measurements: [fabric research](fabrics.md). Fabric-101 does not supply a speedup for the current solver merely by being cited.

## Implemented optimization

`Bvh::pairs_with` traverses point and triangle hierarchies together. `self_pairs` traverses edge-node pairs once. Whole distant node pairs and fixed/fixed pairs are rejected before visiting individual primitives. Point boxes, reciprocal fixed-point/moving-face pairs and moving-edge/fixed-edge pairs remain covered regardless of primitive ID order. Leaf filtering uses a conservative AABB distance; exact geometric evaluation still follows.

There is no fixed candidate capacity or heuristic normal/curvature culling. The same initial separation assumptions and local seam exclusions apply. Two Rayon jobs handle VF and EE generation; the fixed-step worker, mesh, dt, iterations and GPU force law are unchanged. `tests/ogc_bvh.rs` checks pair sets and uniqueness against exhaustive search at several radii after deformation, with fixed IDs interleaved. Existing tests compare cached contacts/radii to fresh detection.

The next substantial stage remains resident GPU broad/narrow phase with explicit overflow handling and f64 comparison. For stiff fabrics and better seam closure, compare augmented/global methods on the same scenes and quality metrics before changing the backend. Do not advertise a universal FPS figure from one short window test.
