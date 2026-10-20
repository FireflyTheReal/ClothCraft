# Redundant-contact Newton search

CPU research follow-up to [the discrete-force pilot](discrete-contact-solve.md).
The production editor and VBD/GPU solver are unchanged.

## Captured failure

The first rejected Newton matrix on the eased synthetic fixture at step 11,
iteration 0 has 65 unknowns and numerical rank 64. The prior strict elimination
rejects it. A separately computed SciPy/LAPACK SVD solution has positive proposed
reactions, force residual 1.33e-13 N and maximum geometric linear residual
4.11e-18 m. Thus rejecting every singular matrix unnecessarily discards a useful
search direction on this case. This does not prove all redundant systems are
consistent or physically feasible.

The public synthetic capture and independent reference vector are in
`tests/fixtures/discrete-contact-newton*.json`. No user garment data is embedded.

## Independent Rust implementation

`contact_newton::least_squares` uses Householder QR with column pivoting, then
Householder QR of the retained triangular block's transpose to obtain a
minimum-norm least-squares solution. Column norms are recomputed at each pivot.
It avoids normal equations, diagonal shifts and contact compliance. All original
rows contribute to the reported linear residual, including inconsistent rows.

The numerical reference consulted was
[LAPACK DGELSY documentation](https://netlib.org/lapack/explore-html/dc/d8b/group__gelsy_ga6d1d46ead18df76e993cd4eda6dc1bbb.html).
It describes complete orthogonal factorization for rank-deficient least squares.
This implementation is independent and uses a simpler trailing-column norm rank
criterion, not DGELSY's incremental condition estimate. It is not a LAPACK port
or a guarantee of strong rank revelation for every matrix.

The research flag `--rank-newton` requires `--discrete-forces --newton-forces`.
Newton force rows are scaled by 1e-7 N and geometric rows by 1e-10 m, matching
existing globalization residual scales. The relative rank threshold is 1e-12.
This changes the search direction only. Nonnegative reaction checks, full
nonlinear force/complementarity gates, OGC bounds and swept collision certificates
are unchanged. An inconsistent linearization may yield a trial direction; its
nonzero residual is reported, never treated as proof of physical convergence.

## Physical convergence versus search-model convergence

The pilot previously required a small displacement from the **geometric search
QP** even after the actual discrete-force balance and complementarity were
satisfied. The captured step reached a physical residual of 5.92e-9 N while the
QP still proposed 1.11e-8 m. That extra criterion tests a different force model
and can reject a valid physical state. Discrete-force mode now checks the full
physical force norm, complementarity and every endpoint gap; the endpoint
baseline retains its original correction-norm test.

An unconverged inner QP can now supply a trial proposal in discrete-force mode.
Its failure is retained in `failed_qp` and counted in `inexact_search_qps`. It
cannot certify a timestep: proposals must decrease the actual merit, satisfy
OGC and every swept test, and ultimately meet the unchanged physical force
(1e-7 N), complementarity (1e-12 J) and geometric (1e-10 m) tolerances. This is
not reclassifying an inner QP as solved or loosening collision acceptance.

## Validation

Kernel tests cover nonsymmetric blocks, scaled consistent and inconsistent
rank-deficient systems, minimum norm under column mixing, rectangular systems,
invalid inputs, and the independently computed captured SVD solution. The SVD
reference uses SciPy 1.15.3 / NumPy 1.26.4 with `lstsq(..., cond=1e-12,
lapack_driver="gelsd")`; it is only an offline oracle, not a runtime dependency.

Trajectory results and reproduction commands are recorded with the local
`output/contact-rank-20261020` evidence. A linear solve or isolated timestep is
not a complete pull/release, dress, performance or jitter validation.

## Trajectory results

[Machine-readable validation](contact-rank-validation.json) retains all four
180-step protocols, including incomplete/failed runs. The final binary gives:

| Seams | Release | Completed steps | Full quality gate |
| --- | --- | ---: | --- |
| Matching | Moving | 87/180 | Fail during pull: nonnegative reaction transition |
| Matching | Held | 87/180 | Same pre-release failure |
| 10% eased | Moving | 180/180 | Pass |
| 10% eased | Held | 133/180 | Fail three steps after release |

The passing case retains minimum separation 1 mm within roundoff, force residual
below 9.92e-8 N, complementarity below 1.99e-13 J, and maximum post-release energy
increase 8.78e-15 J. Two inner QPs report incomplete convergence; no final
physical timestep does. Its controlled vertex moves 33.84 mm during the pull.
Whole-patch displacement RMS after release is 1.22 mm at the next step and
8.28 mm after ten steps, so passing is not achieved by freezing the state.
This frictionless, unpinned fixture can retain rigid motion; total kinetic
energy need not reach zero. Perceptual fluidity or whole-dress jitter is not
established by these measurements.

The previous captured step 11 now completes when checked against the actual
force equations. A separate legacy-checkpoint held replay advances 73 steps
from step 60, then fails at 133. These checkpoint experiments are additional
negative controls, not substitutes for the full matrix above.

The successful final run costs about 89.5 ms per step in one CPU development
run including diagnostics, on only 16 vertices. This is not realtime. No
production performance comparison is justified across the different trajectories.
The old endpoint baseline reproduces both recorded isolated positions and
velocities exactly with the final executable. Twenty-eight targeted CPU tests
and all-target CPU Clippy pass.

Next work: handle contact activation/deactivation and nonnegative multipliers
inside the coupled search. At the remaining failures, the unconstrained Newton
proposal requests negative reactions and is rejected. A consistent active-set
or complementarity solve must resolve that transition while retaining every
geometric inequality, the full residual and swept collision tests. Do not fix it
by accepting attraction, relaxing clearance or adding damping/sleep.

## Reproduction

Run `output/contact-rank-20261020/reproduce.ps1` from the repository root after
building the CPU example. It runs the four full protocols using the archived
final executable. Inspect each JSON `failure` and the full quality report;
a process exit of zero does not mean the physical protocol passed.
Source, inputs and binary hashes are in `output/contact-rank-20261020/build.json`.
Earlier `capture`, `step11`, `eased-moving` files are preliminary controls from
before the coherent physical convergence checks; `final-ease*` are authoritative.
