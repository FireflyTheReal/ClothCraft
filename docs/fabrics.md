# Fabric library, mass and stretch

In the inspector, select a piece and open **Tissu · poids et stretch**. Choose a preset, adjust its mass, thickness and extension, then **Appliquer le tissu**. The default scope is the selected panel; **Appliquer à tous les panneaux** is explicit. Editing one panel detaches its shared material when necessary. Applied values participate in undo/redo and template/project JSON export.

The displayed panel/garment mass is the sum of lumped vertex masses from rest area × surface density; it excludes accessories, seam thread and the avatar. Changing fabric rebuilds masses, energies and solver caches while preserving compatible geometry and velocities. It does not remesh, change material UVs or overwrite the panel grain. This is an instantaneous parameter edit, not a mass-transfer experiment conserving momentum.

## Presets and evidence

**All elastic, shear and bending settings in this first library are authored approximations. None is a calibrated material model.** Mass and some thickness values come from specific measured specimens or a supplier specification. They are not universal values for a fibre family. In particular, woven cotton, jersey and elastane blends must be distinguished.

The MIT workbook was downloaded and read, including its README. Values below average its available specimens, then convert `oz/yd² × 33.905747` to g/m² and `in × 25.4` to mm. [Bouman et al., MIT Fabric Properties Dataset, ICCV 2013](https://people.csail.mit.edu/klbouman/pw/projects/materialproperties/dataset.html). Jersey mass comes from Gray Interlock in Table 1 of [Wang, O'Brien & Ramamoorthi, SIGGRAPH 2011](https://wanghmin.github.io/publication/wang-2011-dde/Wang-2011-DDE.pdf). Leather uses [PURLEDER Auto Nappa](https://purleder.com/en/our-leathers/autonappa); its 0.9–1.1 mm range is represented by 1 mm.

| Preset | Mass g/m² | Thickness mm | Source for mass/thickness | **Estimated** extension at 100 N/m, warp / weft |
|---|---:|---:|---|---:|
| Light woven cotton | 99.648 | 0.1905 | MIT #5 | 3 / 5 % |
| Cotton blend jersey | 187 | 0.6 | Wang Gray Interlock, 60% cotton / 40% polyester; thickness estimated | 15 / 35 % |
| Heavy woven wool | 384.580 | 1.1201 | MIT #6 | 6 / 9 % |
| Denim | 296.675 | 0.6096 | MIT #13, one specimen | 1.5 / 3 % |
| Heavy nappa leather | 750 | 1.0 | Supplier, leather goods/upholstery reference | 4 / 6 % |
| Light silk | 66.968 | 0.1270 | MIT #3 | 3 / 5 % |
| Linen | 151.787 | 0.2286 | MIT #8 | 2 / 3 % |
| Nylon ripstop | 58.516 | 0.0762 | MIT #25 | 4 / 6 % |
| Stretch knit / Lycra | 239.405 | 0.3810 | MIT #1 | 40 / 60 % |

The MIT beam bending measurements have units **lbf·in²**, not shell rigidity N·m. They have deliberately not been copied into `bend_warp/weft`: a compatible specimen-width/model conversion and validation are required. The source image also differs from the workbook in some entries; use the workbook for this library's mass/thickness. Leather directions represent two local material axes; leather has no woven warp/weft. Its approximation does not model hide-to-hide variation, plasticity or damage.

## What “stretch” means here

The control specifies engineering extension under **5 N on a 50 mm strip**, equivalent to nominal traction 100 N/m. It is a convenient, explicit comparison load, not a claim of compliance with a textile test standard. It is neither breaking elongation, a hard extension limit, nor an arbitrary solver multiplier.

`fabrics.rs` converts this extension into the membrane modulus using the **actual smoothed Green-strain law** in `energy.rs`. For stretch λ and Green strain e=(λ²−1)/2, nominal traction is dW/dλ. Solving for the modulus at the chosen λ ensures the UI matches the constitutive energy, including its compression transition. The inverse mapping displays imported coefficients without changing them. CPU tests compare the resulting traction against an independent finite difference of membrane energy.

Advanced controls expose shear in N/m and bending in µN·m. Density is stored in kg/m² (`FabricProperties.density`), thickness in metres. Material thickness does **not** automatically modify the OGC contact radius; these are different parameters. Presets preserve the current compression law. They do not add yarn physics, hysteresis, friction or a knit-specific constitutive model. Stiff materials can require more iterations for accurate deformation; eight iterations is not a convergence guarantee.

## Research read for this pass

- [Wang et al., 2011](https://wanghmin.github.io/publication/wang-2011-dde/): inspected the paper, its measured specimen table and nonlinear/anisotropic modeling discussion. A future calibrated library should fit multi-direction force–extension curves to our law or add a compatible data-driven law. Its complete parameter model is not implemented here.
- [Bouman et al., 2013](https://people.csail.mit.edu/klbouman/pw/projects/materialproperties/dataset.html): read the physical-measurement README, spreadsheet and specimen identification table. Supplies specimen mass/thickness references; no tensile curves in this workbook.
- [Fabric-101 / Learnable Persistent Wrinkle Formation, September 2026](https://arxiv.org/abs/2609.13707): read the abstract and [official repository](https://github.com/GongDeshan/Fabric_101_for_Wrinkles). It studies cyclic elastic, frictional and plastic behavior. At inspection on 2026-10-14 the README still marked the complete dataset/code release as coming soon. No claim of importing its parameters or reproducing its results.
- [UK Hide nappa specification](https://www.ukhide.co.uk/wp-content/uploads/2017/05/Auto-Full-Grain-Nappa-technical-specifications.pdf): the 35–70% figure is **breaking elongation**. It was not used as recoverable stretch at our reference load.

Implementation: `src/fabrics.rs` (pure CPU library and load conversion), `authoring::Command::Fabric` (atomic assignment), `src/atelier/fabrics_ui.rs` (controls). No file-format version bump or changes to existing default materials. Custom numerical properties remain serializable independently of the built-in list.
