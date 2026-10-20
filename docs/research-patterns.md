# Patterns, sewing and fiber mapping — review of 14 October 2026

A targeted primary-source review to guide modules, not a reproduction of the papers or an exhaustive ranking. Existing implementations are independent and documented in [template-format.md](template-format.md).

| Source consulted | Useful result | Consequence for this repository |
|---|---|---|
| [GarmentCode, SIGGRAPH Asia 2023](https://korosteleva.com/publication/garmentcode/) and [authors’ repository](https://github.com/maria-korosteleva/GarmentCode) | Composable parametric patterns, with pieces separated from sewing interfaces. | Keep semantic IDs and persistent panel/seam relationships separate from the mesh. Rectangular v1 is not an implementation of their DSL or a format importer. |
| [True Seams, Rodríguez & Cirio, 2022](https://gabrielcirio.gitlab.io/projects/trueseams/trueseams.html) | Real seams depend on overlap, folds and stitches; construction uses 2D problems then lifts to 3D. Page/abstract consulted. | An edge connection does not reproduce these properties. A future seam-construction layer needs geometry and contact, not merely increased stiffness. |
| [Automatic Digital Garment Initialization, Liu et al., 2024](https://wanghmin.github.io/publication/liu-2024-adg/) | Initial arrangement without folds/intersections is distinct from simulation, handled through classification, heuristics and optimization. Page/abstract consulted. | Retain editable placement per panel. Numerically closing two pieces does not assemble arbitrary garments around an avatar. |
| [Computational Pattern Making from 3D Garment Models, 2022](https://arxiv.org/abs/2202.10272) | Patterns from 3D surfaces with manufacturing/deformation constraints. Listing/abstract consulted. | A future 3D→2D conversion must manage distortion and grain; rendering UVs are not automatically physical patterns. |
| [EASE, Bartol et al., 2026](https://arxiv.org/abs/2606.29419) | Local anisotropic ease as a design variable distinct from physical strain; constrained pattern optimization. Listing and sections 3.1–3.3 consulted, Computer & Graphics vol. 138. | Separate allowance/ease from elasticity. Our `allow_ease` only permits unequal boundary lengths; no EASE model is implemented. |
| [Homogenized Yarn-Level Cloth, Sperl, Narain & Wojtan, 2020](https://research-explorer.ista.ac.at/record/8385) | Yarn-pattern simulations produce a macroscopic energy density for a shell. Listing/abstract and indexed excerpts consulted; project publisher PDF opening failed. | A coherent route to keep FEM while replacing the material law with a yarn-derived law. Current parameters do not come from this procedure. |
| [Neural-Assisted Homogenization of Yarn-Level Cloth, Feng et al., SIGGRAPH 2024](https://wanghmin.github.io/publication/feng-2024-nah/) | Constitutive law learned from deformation energies; stability and data collection matter. Page/abstract consulted. | A learned law needs Newton-compatible energy/derivatives and out-of-distribution validation. Preparing that interface does not require training. |

The consulted [neural_yarn repository](https://github.com/Rullec/neural_yarn) showed only a README; it does not by itself provide a verified reusable implementation. [Neural Geometry Details for Real-Time Multi-Scale Cloth Simulation](https://sueraywang.github.io/project/nngeomclothdetail/) is presented as a proposal/project log, not evidence here of a validated scientific result.

In [EASE §3.2–3.3](https://arxiv.org/html/2606.29419v1), barycentric constraints are frozen after LSCM initialization; local/global optimization adjusts the pattern with length compatibility and seam regularization. XPBD draping follows and merges seam vertices. This design pipeline differs from our mechanical assembly with separate vertices. The [public EASE/LOOM README](https://raw.githubusercontent.com/kristijanbartol/ease/main/README.md) describes a Python/C++ pipeline, SMPL dependencies and manufacturing limits. It was read; code was neither executed nor audited.

## Architectural decision

Turning a 2D sheet into cloth has three levels:

1. **Material geometry:** pattern contour, grain, fabric and assembly. Rest geometry remains 2D through 3D placement and resolution changes.
2. **Effective mechanics:** F transforms warp/weft directions; an energy law provides forces. The current analytical law is a reference to replace or calibrate, not an identified industrial fabric.
3. **Fine structure:** paths defined on the pattern attach to the surface through barycentric coordinates. This supports frames, stitch paths and visual detail. Actual yarn simulation also needs textile topology, yarn bending/torsion, contacts and measured parameters.

The initial modules covered level 1 for rectangles, mechanical connections and level 3 geometric attachments. They did not add physical yarns. Homogenization is a direction for level 2, not an enabled backend.

## Proposed CPU experiments at the time of this review

- Oriented polygon contours, constrained triangulation, stable boundary IDs and seam-transfer tests under refinement. These foundations have since been added.
- Separate cut lines, sewing lines and allowances, with offset/intersection tests on small contours.
- A measured constitutive reference: explicit data, units and protocol; validate gradients/Hessians and objectivity before integration.
- Compare resolutions and seam penalties with reliable contact before claiming faithful gathers or layered thickness.
- Initial arrangement and CPU VF/EE contact before extending examples to a sleeve or closed layers. See the later [OGC implementation](ogc.md).

## Reuse and provenance

The [GarmentCode root license](https://raw.githubusercontent.com/maria-korosteleva/GarmentCode/main/LICENSE) was read: MIT, copyright 2024 Maria Korosteleva. No code from that repository or these papers was copied. Dependencies, models and data were not audited. References on `main` move; pin a commit and inspect notices before reuse.
