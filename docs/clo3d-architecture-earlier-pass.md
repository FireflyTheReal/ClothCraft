# CLO3D Architecture & Workflow — Reverse-Engineering Dossier

**Target:** CLO 2026.1.310 (`/Applications/CLO.app`)
**Binary:** `Contents/MacOS/CLO_Standalone_OnlineAuth`, 126 MB, universal `x86_64` + `arm64`
**Method:** static analysis with Ghidra 12.1.4 through REA; symbol-table recovery;
per-library Mach-O inventory; string and dependency analysis
**Date:** 2026-10-15

---

## 0. Scope, method, and what this document is not

This dossier records **structure and design** of the CLO3D application. It is
written for interoperability and independent reimplementation. It is explicitly
**not** a line-by-line transcription of CLO's code, and no CLO source,
decompiled function bodies, assets, shaders, or licensing internals are
reproduced into ClothCraft. Where this document describes an algorithm, it
describes the *published technique* that the evidence shows CLO uses, and
ClothCraft implements that technique independently from the literature.

Evidence discipline used throughout:

* **Observed** — read directly from the shipped artifact (symbols, decompiled
  pseudocode, strings, load commands).
* **Derived** — a conclusion that follows from observations.
* **Unknown** — not established here, and not guessed.

Evidence IDs from the analysis session are cited inline. Addresses are
provider-normalized for the thinned `arm64` slice; the original artifact is
never modified.

---

## 1. Application shape

CLO3D is a **Qt 6 desktop application** (Widgets plus QML/Quick), not an
Electron or web app. Load commands show direct linkage to `QtWidgets`,
`QtGui`, `QtCore`, `QtQuick`, `QtQml`, `QtWebEngine*`, `QtOpenGL`,
`QtMultimedia`, `QtSvg`, `QtXml`, `QtNetwork`, `QtConcurrent`, `QtPrintSupport`,
`QtSerialPort`, `QtPositioning`, and `QtDBus`.

Derived: the 2D pattern editor is Qt's graphics-view framework, and the 3D
viewport is a Qt `QOpenGLWidget`-hosted renderer. A QtWebEngine runtime means
some panels are HTML/JS, and `QtQml` means part of the UI is declarative.

Two architecture markers worth noting for a reimplementation:

* `libCloWrapper.dylib` contains ~1,700 `cw*` symbols (`cwstring`,
  `cwvariant`, `cwjsonobject`, `cwfile`, `cwurl`, `cwtransform`, …). These are a
  thin **Qt-subset compatibility shim** — the engine is written against an
  abstract string/container/variant layer rather than Qt directly, and the
  shim binds it to Qt. A reimplementation gets this separation for free by
  choosing its own core types.
* `libomp.dylib` plus TBB (`libtbb.12.dylib`, `libtbbmalloc*`) show the engine
  is parallelised with **OpenMP and TBB**, which is confirmed in the solver
  (see §5).

### 1.1 Subsystem decomposition (from the 155-entry `Frameworks/` inventory)

| Library | Role (observed) |
|---|---|
| `libCloScene.dylib` (194 MB) | Scene graph, document model, editing core |
| `libCloMath.dylib` | Geometry, linear algebra, serialization primitives |
| `libCloCore.dylib` | Threading / global task arena |
| `libCloWrapper.dylib` | Qt-subset compatibility layer (`cw*`, tinyxml2) |
| `libCloSimulate{Cuda,Metal,RootParity,EBP,WRAPD,Solvers,Physics,DataStructures,Common,Utils}` | Cloth solver stack (see §5) |
| `libCloKnit_{Core,SwatchEditor}` | Knitwear authoring (stitch-level, distinct domain) |
| `libCloAutoRetopology`, `libgeometry-central` | Mesh remeshing / retopology |
| `libCloDeforMarvel` | Legacy Marvelous deformation, shared engine lineage |
| `libCloSketch`, `libCloCV` | Sketch tools; OpenCV-backed CV |
| `CLOAPIInterface`, `libCLOAPIInterface` | Automation surface |
| `libvray*.dylib` (≈450 MB total) | V-Ray production renderer + OSL + OIDN denoise |
| `libusd_ms`, `libAlembic`, `libfbxsdk`, `libcollada-dom` | Interchange (USD, Alembic, FBX, COLLADA) |
| `libonnxruntime` | Neural inference |
| `libPDFNetC`, `libscatter` | PDF output; fabric scatter/weave simulation |
| `libhasp_darwin`, `libcgauth` | Hardware dongle + online licensing |
| `libzstd`, `liblzma`, `libbrotli*` | Compression (project/asset containers) |
| `libCloSimulateRootParity` | Root-motion parity (animation retarget) |

---

## 2. Engine object model

64,000+ symbols were recovered across the executable and libraries; the main
binary alone carries **32,792 defined text symbols** and is **not stripped**.
The engine's top-level C++ namespace is **`Marvelous`** (6,374 symbols), which
establishes that CLO and Marvelous Designer share an engine lineage.

`Marvelous::` second-level classes, by frequency, recovered from the main
binary:

```
570 PatternEditor      438 MVProperty        343 Fabric         295 Pattern
273 UnReMultipleOperations  261 AvatarRoot    221 Shape2D        213 ThickParametricLine
211 MVAnimUnit         195 Camera            194 Editor2D       184 PBRMaterialGUIInterface
176 MaterialProxy      174 Element           169 PatternSnapShot 165 AvatarManager
157 ZipperStyle        154 TopStitchStyleProperty             153 ModularLibraryDataManager
143 MVColor            134 AnimationController                 129 Transformer3D
128 NestingProperty    125 Texture           125 ColorwayLibrary
122 PBRMaterialPublicInterface           117 MVVariantMap    110 PuckeringProperty
108 Zipper              98 PatternList        98 KnitEditorWindow 96 StitchProperty
```

The UI layer is a second namespace, **`CloUICommon`** (3,448 symbols), plus
`View2DActionSlots` (407) and `View3DActionSlots` (350) — i.e. the action/command
surface is split by viewport.

### 2.1 The 2D pattern editor — key finding

The pattern editor is **not** a polygon mesh editor. It is built out of:

| Type | Symbols | Inferred role |
|---|---|---|
| `PatternEditor` | 570 | Editing session over a pattern |
| `ThickParametricLine` | 213 | **Parametric curve with a thickness** — a boundary edge is a curve with width, not a bare polyline |
| `Shape2D`, `Line` | 221 / 107 | 2D geometry primitives |
| `Pattern`, `PatternList`, `PatternSnapShot` | 295 / 98 / 169 | Pattern model, collections, undo snapshots |
| `Editor2D`, `View2DActionSlots` | 194 / 407 | Viewport and its command surface |
| `SeamLinePairGroup` | 87 | A seam is a **pair of line groups**, i.e. many-to-many, not one edge pair |
| `StitchProperty`, `PuckeringProperty` | 96 / 110 | Per-stitch and per-seam physical properties |
| `GradingRuleTable`, `GradingWidget`, `GradingReviewMain` | 85 / 112 / 148 | Grade rules — the file's size/grade structure |

Derived design consequence for a reimplementation: a garment pattern is best
modelled as **curved parametric boundary segments carrying a width, joined into
closed pieces, with seams joining *ranges* of segments**. A seam joining one
segment to one segment is a special case, not the general case. ClothCraft's
`clothcraft-pattern` crate models pieces and segments and already carries a
`SeamRef` with `first`/`last` indices; it should grow toward the
segment-*range* pairing this evidence implies.

### 2.2 Materials, trims, and garment features

`Fabric` (343) plus `MaterialProxy` (176), `PBRMaterialGUIInterface` (184),
`PBRMaterialPublicInterface` (122), `Texture`/`TextureMapType` (125/85),
`Colorway`/`ColorwayLibrary`/`ColorSwatch*` (~230) show a **PBR material system
with named texture-map slots and a first-class colourway concept**.

Garment-feature families recovered: `Zipper`/`ZipperStyle`/`ButtonHeadStyle`
(~700 symbols combined — a very large component), `TrimObjectStyle` (103),
`NestingProperty` (128), `Pleat` (81), `Dart` (103), and garment-level objects
such as `GarmentInfoItem`, `EveryWearModel`, `ModularLibraryDataManager`.

Derived: trims (zippers, buttons) are *modelled objects with their own styles
and registration dialogs*, not textures faked onto the cloth. This is a large
amount of product surface, and it is where a clone would spend most of its
effort without adding simulation value.

### 2.3 Avatar

`AvatarRoot` (261), `AvatarManager` (165), `Transformer3D` (129), plus
`DialogCloAvatarEditor` (338) and `DialogAIAvatarStudio` (100). On disk the
avatar presets are **`.obj` base meshes** with separate `Tetrahedralize/`
inputs (`female_ordered_cap_hole.obj`) and `Converting/` `.avt` files
(`Genesis8_Female.avt`, `Genesis9_Male.avt`, …).

Observed/derived: avatar sizing runs on an ordered base mesh that is
**tetrahedralised** for volumetric fitting, and `.avt` is a conversion/target
profile (the names are DAZ Genesis characters). A `.zpac` "SkinSuit" preset
sits alongside them.

---

## 3. File formats

Format tokens recovered from the main binary's string table:

```
$Project (ZPRJ) / Garment (ZPAC) File
ZPAC (Zipped File including PAC and Texture Images)
:Files exceeding 2GB cannot be saved as a single ZPRJ file.
Colorway Compatibility for 7.3 and 2024.0 (ZPRJ)
.avt   .zfab   .zpac   .zprj
```

| Extension | Observed meaning |
|---|---|
| `.zprj` | The **project** (document) container. Has an explicit 2 GB single-file cap, and explicit version-compatibility branches (7.3, 2024.0) for colourway data. |
| `.zpac` | A **ZIP container** holding the pattern and texture images. Ships as a preset `SkinSuit.zpac`. |
| `.zfab` | A **fabric/material** asset (`Preset/Bond/*.zfab`). |
| `.avt` | An **avatar** profile. |

### 3.1 The serialization primitive — key finding

`libCloMath.dylib` exports `Marvelous::MVVariant`, a tagged variant with:

```
MVVariant(bool|char|int|float|double|int64|uint64|string|bytearray|vec2|vec3|vec4|mat3|mat4)
MVVariant::readFromFile(FILE*, bool)
MVVariant::readFromFile(FILE*, unsigned int)      // versioned read
MVVariant::readFromData(std::vector<char> const&)
MVVariant::readFromByteArray(unsigned char const*, unsigned long long)
MVVariant::toByteArray(...) / toArray / toMap / toList / toInt / toDouble / toFloat
MVVariantArray, MVVariantList, MVVariantMap
MVVariant::getSizeInfo(...)   // seven out-params — nested size accounting
```

Derived: `.zprj`/`.zfab`/`.avt` are a **tagged-variant tree with a version
word**, block-addressed from a `FILE*`, with nested containers
(`Array`/`List`/`Map`). Every subsystem's `Load`/`Save` takes an
`MVVariantMap` — for example `PhysicalProperty::Load(MVVariantMap const&, bool)`
and `WindProperty::Save(MVVariantMap&)` in `libCloSimulatePhysics`. This is the
backbone of every asset format, and it is **not** documented publicly.

Because `MVVariant` is a container format rather than a semantic schema, a
clean-room reader is feasible in principle, but the *schema* (which key means
what) is the real reverse-engineering work and is only partially observable
here. ClothCraft does **not** parse these formats; it stays on open formats
(HPGL, DXF) and records this as an open problem.

---

## 4. Automation surface

`CLOAPIInterface` / `libCLOAPIInterface.dylib` (121 KB, 21 text symbols) plus a
bundled CPython 3.11 (`libpython3.11.dylib`, `Resources/python311.zip`) mean the
app exposes a **Python scripting API** for automation, distinct from the deep
engine ABI. This is the safest interoperability path: driving the shipped
application through its own public API rather than reimplementing or patching
it.

---

## 5. The cloth solver — recovered design

This is the most valuable section, because it is where behaviour is
unambiguous and where the evidence is strongest.

### 5.1 Evidence

`libCloSimulateSolvers.dylib` (thin `arm64`, SHA-256 `3050227736cf…`) contains
only 35 procedures and is therefore fully enumerable. Its exports are the
linear-solver core:

```
Marvelous::CGE(matMixCSR&, symMat3*, vec3*, uint n, uint maxIter, float tol,
               vec3*, vec3*, vec3*, vec3*, vec3*)
Marvelous::ComputeAD(matMixCSR&, vec3 const*, uint, vec3*)
Marvelous::ComputeAD_CPU(...)                     // OpenMP driver (__kmpc_fork_call)
Marvelous::ComputeInverseAii(matMixCSR&, uint, symMat3*)
Marvelous::ConjugateGradient_CPUE(...)            // thunk to the GPU/CPU pair below
Marvelous::ConjugateGradient_CPUE(..., bool, ...)
```

Decompiled (`batch_decompile`, addresses are for the thinned slice):

**`ComputeAD` @ `0x34d4` — the sparse 3×3-block matrix-vector product.**
It zeroes `n` 3-vectors, then per row `i` accumulates:

```
y_i = A_ii * x_i + Σ_j A_ij * x_j
```

*The **diagonal** block is read as **six** floats at stride `0x18` (24 bytes)
and contracted as a symmetric matrix — confirming lower-triangular
`(m00, m10, m20, m11, m21, m22)` storage.
*The **off-diagonal** block is read as **nine** floats at stride `0x24`
(36 bytes) — a full symmetric 3×3.
*Row pointers are `int32` at offset `+0x10`, column indices at `+0x40`.
This mixed-size, two-value-array-plus-shared-row-pointer layout is what the
name `matMixCSR` denotes.

**`ComputeInverseAii` @ `0x3684` — the block-Jacobi preconditioner.**
It walks the diagonal blocks and calls the external `invert(symMat3&, symMat3
const&)` from `libCloMath` on each. So the preconditioner `M` is *exactly the
block-diagonal part of A*, inverted.

**`CGE` @ `0x2848` — preconditioned conjugate gradient.**
Its structure is the textbook PCG:

```
r    = b - A x                       (via ComputeAD)
rho  = r . r
eps  = max(1e-15, rho * 1e-15)
while iter < maxIter and rho > eps:
    Ap     = A p                     (via ComputeAD)
    alpha  = rho / (p . Ap)
    x     += alpha p
    r     -= alpha Ap
    z      = M^-1 r                  (diagonal-block solve; direction unchanged as r)
    rho'   = r . z
    beta   = rho' / rho
    p      = z + beta p
    rho    = rho'
```

The decompiler output shows explicit **ARM NEON** intrinsics (`NEON_ext`) in the
hot loop and an OpenMP-parallel `ComputeAD_CPU` driver — the inner kernels are
hand-vectorised and threaded.

### 5.2 Interpretation

CLO's implicit cloth timestep is a **block-sparse symmetric system with 3×3
nodal blocks, solved by block-Jacobi-preconditioned conjugate gradient**. That
is the standard implicit/backward-Euler cloth formulation. Combined with the
physics library's symbols:

```
DeforMarvelSpring(uint, uint, uint, float, float, float)   // spring with rest + stiffness + damping
NonlinearStiffness::Load/Save(MVVariantMap&)
PhysicalProperty::Load/Save/ApplyMinConstraint/InterpolatePP
WindProperty::{GetPosition,GetRotation,GetDecay,GetTurbulence,GetBoundingVolume,…}
```

the material model is a **mass-spring system whose stiffness is a nonlinear
(load-dependent) curve**, serialised as piecewise functions
(`ToNonlinearStiffnessPiecewiseFn`), plus a wind field and per-property minimum
constraints.

The spatial acceleration structure is also here, in
`libCloSimulateDataStructures.dylib`:

```
KDTree::construct / constructSubtree / FindNearestNeighbor / getSquaredDistanceToBox
KDTreeInternalNode::Search, KDTreeLeafNode::Search, KDSplitAxis
HashTable::CreateEdgeTableFromTriangleMesh(uint, uint*)
HashTable::CreateEdgeTableFromTetrahedronMesh(uint, uint*)
PLIndex  // (piece, line) index pair
```

Derived: collision/broadphase is a **KD-tree over points**, edge adjacency for
both triangle (cloth) and tetrahedron (volumetric avatar/collision) meshes is
built into a **hash table**, and `PLIndex` is the `(piece, line)` pair
identifying a pattern edge — the same identity seam type recovered in §2.1.

### 5.3 What ClothCraft implements from this

| Evidence | ClothCraft |
|---|---|
| `matMixCSR` mixed block-CSR, 6-float symmetric diagonals | `clothcraft-linalg::BlockSparse`, `SymMat3` |
| `ComputeAD` 3×3 block SpMV | `BlockSparse::matvec` |
| `ComputeInverseAii` block-Jacobi preconditioner | `BlockSparse::diagonal_inverse`, `SymMat3::inverse` |
| `CGE` PCG iteration | `clothcraft_linalg::pcg` |
| Nonlinear stiffness curves, mass-spring material | `clothcraft-fabric` |
| XPBD/published constraint formulation for the explicit path | `clothcraft-solver` |

The new `clothcraft-linalg` crate is written from the published PCG method and
the general block-sparse idea — not transcribed from the decompiler output. Its
tests check the mathematics (inverse round-trip, residual reduction against a
dense solve, structural validation), not byte-equality with CLO.

---

## 6. Workflow, end to end

Reconstructed from the command surface (`View2DActionSlots`,
`View3DActionSlots`, `MainMenuActionSlots`, `DockWidgetController`,
`DialogController`) and the object model:

```
1.  New/open project ────────────► .zprj (MVVariant tree, versioned)
2.  Choose or size avatar ───────► BaseMesh.obj ──tetrahedralise──► collision body
                                   .avt = conversion profile
3.  Draft the 2D pattern ────────► PatternEditor over ThickParametricLine curves
     (or import DXF/HPGL)          Shape2D + Line, Pattern/PatternList
                                   MeasurementTable drives parametric drafting
                                   PatternSnapShot = undo
4.  Join pattern edges ──────────► SeamLinePairGroup (segment *ranges*)
     StitchProperty, PuckeringProperty, TopStitchStyleProperty
5.  Assign materials ────────────► Fabric + PBRMaterial* (texture-map slots)
                                   Colorway/ColorwayLibrary
6.  Add features ────────────────► Zipper/Button/Trim/Pleat/Dart objects
7.  Place on avatar ─────────────► 2D pieces ──stitch──► 3D shell
8.  Simulate ───────────────────► mass-spring + nonlinear stiffness
                                   implicit step: matMixCSR PCG (§5)
                                   wind, collision (KD-tree), self-collision
9.  Grade / size ────────────────► GradingRuleTable, GradingPointProperty
                                   NestingProperty for marker making
10. Render ──────────────────────► V-Ray (production) or viewport GL/Metal
11. Export ──────────────────────► OBJ, USD, Alembic, FBX, COLLADA, PDF, image
                                   ExportOBJ/ImportOBJ dialogs
```

Cross-cutting: `AnimationController`/`MVAnimUnit`/`MVVariantMap` (animation and
data binding), `UnReMultipleOperations` (undo/redo), `AuthenticationProcessor`
plus HASP dongle (licensing), `CLOUIUpdateImpl` (updates).

---

## 7. Open problems and unknown areas

Recorded honestly rather than guessed:

1. **`.zprj` schema.** `MVVariant` gives the container mechanics; the key
   vocabulary is only partially visible. A full reader needs systematic
   observation of files produced by the application across versions.
2. **The EBP / WRAPD / RootParity solver variants.** `libCloSimulateEBP`,
   `libCloSimulateWRAPD`, and `libCloSimulateRootParity` name specific
   algorithms whose exact formulations were not decompiled here. EBP and WRAPD
   are plausible references to published projective/domain-decomposition
   methods, but that is an inference, not an observation.
3. **Fabric parameter space.** `PhysicalProperty` fields were enumerated, but the
   preset library's actual numeric ranges were not extracted.
4. **GPU paths.** `libCloSimulateCuda` and `libCloSimulateMetal` exist; the
   kernels were not examined (the CUDA path is not loadable on this host).
5. **Knit domain.** `libCloKnit_Core` (248 symbols) is a genuinely separate
   stitch-level model that this pass only surveyed.
6. **Whether any of this is needed.** See §8.

---

## 8. Why ClothCraft is not a port

Three independent reasons, in decreasing order of importance:

1. **Most of the value is product surface, not algorithms.** The algorithm
   content that matters (cloth solvers, PCG, KD-trees, mass-spring materials,
   stitch meshing) is *published literature*. The parts that are genuinely
   CLO-specific — the `.zprj` schema, the trim/zipper/button object model, the
   grading tables, the fabric preset library, the V-Ray integration — are
   either unobservable here or are proprietary asset content that must not be
   copied.
2. **The licensed weight is in dependencies, not the engine.** V-Ray alone is
   ~450 MB of the bundle and is commercially licensed. There is no lawful path
   to reproducing it, and no need to: open renderers cover the viewport case.
3. **A byte-level port would be both unlawful and worse.** CLO's speed comes
   from hand-vectorised NEON kernels and a decade of tuning against a specific
   Qt6 + V-Ray + HASP stack. A faithful transcription inherits the design
   constraints (an abstract `cw*` layer, `MVVariant` indirection) without the
   tuning.

What *is* worth building, and what ClothCraft targets, is an independent
implementation of the same **workflow** on open formats: pattern →
seams → assembly → simulation → export, with the solver built from the
literature and the material model driven by measurements rather than by CLO's
preset triples.

---

## Appendix A — Reproduction commands

```bash
B=/Applications/CLO.app/Contents/MacOS/CLO_Standalone_OnlineAuth
nm -arch arm64 "$B" | c++filt | grep -E ' [TtWw] ' | sort -u    # 32,792 symbols
otool -L "$B" | grep -iE 'clo|Qt6|python'                        # link-time deps
lipo -thin arm64 Frameworks/libCloSimulateSolvers.dylib \
     -output /tmp/libCloSimulateSolvers.arm64.dylib              # Ghidra needs a thin slice
```

Ghidra rejects universal Mach-O binaries (`architecture_unsupported`), so every
library must be thinned before `open_binary`.

## Appendix B — Evidence index

| Finding | Evidence |
|---|---|
| Thin solver slice, 35 procedures, 13 segments | `ev_83b737cf…` (`binary_overview`) |
| Full solver procedure/export inventory | `ev_0b63d6e5…` (`list_procedures`) |
| PCG / SpMV / preconditioner pseudocode | `ev_59e11902…` (`batch_decompile` @ `0x2848`, `0x34d4`, `0x3684`, `0x3748`, `0x3c44`) |
| Target identity (SHA-256) | `3050227736cfcd66db262f552660551e735909900e1f8632a301b7c564143446` |

Symbol inventories, dependency lists, string tables, and the file-format
section come from direct Mach-O inspection of the shipped bundle and are not
dependent on the Ghidra session.
