# Edge–edge activation discontinuity

This investigation isolates the 0.619 J refresh event reported in
[solver consistency](solver-consistency-results.md). The production default
remains VBD + the existing OGC potential. The new interior-edge potential is
an **opt-in CPU experiment**, not an editor fix or another cloth solver.
[Measured results and decision](edge-transition-results.md).

The [production VBD follow-up](contact-refresh-follow-up.md) identifies a
separate point–face activation event and adds a public concave-face fixture.
The EE experiment does not resolve that transition.

## Diagnosis

The captured closest-point coordinate on one edge changes from
`s = -0.00048519` (unconstrained line coordinate, outside the segment) to
`s = 0.00029742` (inside). The gap changes from approximately 0.106 mm to
0.158 mm. Before entry, the endpoint fails its vertex feasible-region test.
After entry, the interior EE term is accepted and immediately contributes
0.619052822 J. The parallel-edge mollifier is fully active, so near-parallel
filtering does not explain this event. A simultaneous PF change is only
−1.84e−7 J.

This is a feature-admissibility discontinuity, not missing broad-phase
coverage. The public [IPC Toolkit OGC feasible-region implementation](https://raw.githubusercontent.com/ipc-sim/ipc-toolkit/main/src/ipc/ogc/feasible_region.cpp)
also checks the vertex cone for endpoint contacts and accepts interior EE
contacts. It was actually inspected. Our finding is not a claim that the
paper's separate concave-face limitation explains every jitter event.

The capture now includes the previous detection positions. The new
`edge_transition_probe` interpolates them, checks separation, and records
closest coordinates, violated endpoint tests, energy and gradient norm.
It selects the EE witness from the diagnostic; no private vertex or seam ID
appears in the solver or public fixture.

## Public reproducer

Eight vertices and four triangles suffice, with a connected manifold
vertex fan. One triangle supplies an edge;
another triangle in its endpoint star makes the vertex contact inadmissible.
A separated opposing edge moves past that endpoint. The fresh EE energy
jumps from zero to 0.565853 J even as the displacement tends to zero.
The corresponding four-vertex gradient norm is about 1,378 N.

This geometry also separates contact forces from safety bounds: rejecting a
force never removes the primitive from the movement restrictions. Both energy
models produce exactly the same radii, directional half-spaces and minimum
distances in the synthetic and captured-geometry probes.

## Experimental potential

For nonparallel lines, let `s,t` be their unconstrained closest coordinates,
`u,v` the edge vectors, and `D = |u × v|²`. Define

```
a = sqrt(D / |v|²) / r
b = sqrt(D / |u|²) / r
w = H(s a) H((1-s) a) H(t b) H((1-t) b)
H(z) = 0                           z <= 0
       z³ (10 - 15z + 6z²)         0 < z < 1
       1                           z >= 1
U = w * existing_mollifier * existing_activation(distance)
```

The four ramp arguments are dimensionless projected endpoint distances.
The band width comes from the existing contact radius; there is no
garment-specific parameter. At positive separation, away from parallel
degeneracy, the weight and its first two derivatives vanish at an open
endpoint boundary. The EE contribution agrees with the old one
where all ramps are one. This does not make the entire contact model C2:
the original activation is C1 at its outer radius, other feature families
still switch, and parallel degeneracy retains the existing cutoff.

The experiment retains nearby EE pair identities even outside their interior
support. Their energy is evaluated at every local iterate, so entering that
support does not require adding a finite term at refresh. A scalar support
test skips automatic differentiation when the contribution is identically
zero. That is an evaluation shortcut, not removal from collision coverage.

All derivatives of the weight participate in the same implicit objective,
including tangential derivatives. **This changes the contact force law.** It
can reduce support near endpoints and alter sliding; a smoother energy alone
is not a quality certificate. Existing PP/PE contributions and their switching
remain, so this is not a complete continuous contact formulation. No sleeping,
velocity clipping, added damping, material/rest change, or seam exception is
used. The OGC query envelope, exact geometry and separation regions are
unchanged. The GPU path rejects this experimental model explicitly.

The tapered EE term alone has no uniform barrier guarantee in a simultaneous
endpoint/separation limit. It must not be used as a replacement for geometric
separation checks. The unchanged OGC regions, reciprocal VF coverage and other
contacts remain necessary; retained separation can still come with slow or
stalled relaxation, which must be checked separately.

## Reproduce

```powershell
# Public geometry, derivative and sliding/pull/release regressions; CPU only.
.\scripts\edge-transition.ps1

# Same saved state, VBD settings, avatar and 40 mm pull for both models.
.\scripts\edge-transition.ps1 -Project path/to/garment.clothcraft.json

# A single opt-in run with passive within-step refresh diagnostics.
cargo run --release --locked --no-default-features -j 2 --example solver_compare -- `
  --project path/to/project.json --output output/new-run --backend vbd `
  --steps 120 --iterations 16 --interior-edges --convergence --audit-every-step
```

The script uses two CPU workers and never opens the editor or uses the GPU.
It refuses existing output folders. Inputs are archived portably by the replay
tool; the original save is not changed. `--convergence` adds energy evaluation
work and must be omitted from timing comparisons. CPU VBD refresh deltas are
passive observations, not an energy veto or a prediction rejection policy.

The standard replay lasts two physical seconds at 1/60 s: 0.5 s quiet,
0.5 s pull, 0.25 s hold, 0.75 s release. Each pull has a paired unforced run.
Final seam gap, stretch, panel motion, attachment response and step cost all
matter. The disconnected falling panel is reported separately. A 30-step
highpass tail is a motion diagnostic, not a force-balanced equilibrium test.
Endpoint intersection checks complement the unchanged separation-region
argument; they are not exact continuous collision certification.

## Further work

The next contact-model comparison should cover VV/VE/VF/EE transitions as one
system, including contact support and force balance at boundaries. The
[Geometric Contact Potential project](https://huangzizhou.github.io/research/smooth-contact.html)
specifically studies differentiable geometry-dependent contact potentials and
spurious forces. Its project abstract and
[toolkit README](https://github.com/geometryprocessing/GCP-toolkit) were read;
the full paper could not be fetched in this pass. The toolkit is not a cloth
solver and has not been integrated. The taper above is an independent
experiment, not an implementation of that paper.
