# CLO3D reverse-engineering dossier — assembled nodal timestep operator

**Status:** current pass. Supersedes nothing; it *corrects* and extends the
earlier pass preserved beside it as
[clo3d-architecture-earlier-pass.md](clo3d-architecture-earlier-pass.md).

This document records what was observed in the shipped CLO 2026.1.310 artifacts
with Ghidra 12.1.4 through REA, and how ClothCraft implements the recovered
*technique* independently. It is **not** a transcription of CLO code. No CLO
source, decompiled body, asset, preset, shader or identifier is reproduced into
this repository.

## 0. Evidence discipline

Every claim below is tagged:

* **Observed** — read directly from the shipped artifact (symbol table, load
  commands, decompiler output, string table).
* **Derived** — a conclusion that follows from observations.
* **Inferred** — a plausible explanation that has *not* been verified.
* **Unknown** — not established here, and not guessed.

Evidence IDs are REA Evidence record identifiers from the analysis session, and
they are deterministic: re-running the same operation against the same artifact
and provider profile reproduces the same ID. Two of the earlier pass's IDs were
in fact reproduced byte-for-byte in this pass (see §7).

## 1. Targets

| Target | Identity | Notes |
|---|---|---|
| `/Applications/CLO.app` | `CFBundleShortVersionString` **2026.1.310** | 155 entries under `Contents/Frameworks` |
| `Contents/MacOS/CLO_Standalone_OnlineAuth` | Mach-O 64-bit, universal `x86_64` + `arm64`, 126,231,984 bytes | main executable |
| `clo3d_target/CLO_Standalone_OnlineAuth_arm64` | thinned `arm64` slice, 58,287,536 bytes, SHA-256 `b2d545d7baaefa3b303599c2bb51f49b94b04607531d55ba53f5693e9bff128f` | analysable slice of the main executable |
| `clo3d_target/libCloSimulateSolvers.arm64.dylib` | thinned `arm64`, 69,888 bytes, SHA-256 `3050227736cfcd66db262f552660551e735909900e1f8632a301b7c564143446` | the linear-solver core |

Ghidra rejects universal Mach-O input, so every library is thinned with `lipo`
before `open_binary` (**Observed**: an earlier universal-binary attempt returned
`architecture_unsupported`).

## 2. Method

1. Direct Mach-O inspection (`nm`, `otool`, `strings`, `lipo`) of the shipped
   bundle. This is artifact observation and needs no disassembler.
2. `open_binary` with the Ghidra provider for the small, fully enumerable solver
   library, then `binary_overview`, `list_procedures` and `batch_decompile`.

The main executable carries **50,164** symbol-table entries: **37,211** defined
and **12,953** undefined, of which **32,792** are defined text symbols. It is
**not stripped**, so the engine's structure is recoverable from names alone
(**Observed**, direct `nm`; the Ghidra feature-count route was not needed for
the architecture).

`__text` is `0x25e5e74` bytes (about 39.7 MB) and `LC_FUNCTION_STARTS` is
present, so the symbol table and function-start table agree on function
boundaries (**Observed**: `otool -l`).

## 3. Evidence index

| Operation | Evidence ID | What it establishes |
|---|---|---|
| `binary_overview` (solver) | `ev_83b737cf50fb67a98813d39ab9ec8a0fba899a617ab6075dced8bd2469959dd3` | 13 segments, **35 procedures**, 68 strings, `image_base 0x0` |
| `list_procedures` (solver) | `ev_0b63d6e5ad67f1eb2a4ec71316bca2c73c4dd9de5173f10b113d7164deda6435` | full exported/inventory symbol set and addresses |
| `batch_decompile` (solver) | `ev_ac2b9b3f6fc794c7a713da497c77a2862424b08dd64ae4bebcaa141d30de97b4` | pseudocode for `0x2848`, `0x34d4`, `0x3684`, `0x3748`, `0x3780`, `0x3c44`, `0x3c98`; 7/7 succeeded |

Segments reported for the solver slice: `__TEXT`, `__text`, `__stubs`,
`__cstring`, `__unwind_info`, `__got`, `__const`, `__DATA_CONST`, `__common`,
`__DATA`, two `__LINKEDIT` blocks and `EXTERNAL`.

## 4. The recovered timestep operator

### 4.1 Exported symbols (**Observed**, `ev_0b63d6e5…`)

```
Marvelous::CG(matMixCSR const&, symMat3 const*, vec3 const*, unsigned int n,
              unsigned int maxIter, float tol,
              vec3*, vec3*, vec3*, vec3*, vec3*)                      @ 0x2848
Marvelous::ComputeAD(matMixCSR const&, vec3 const*, unsigned int, vec3*) @ 0x34d4
Marvelous::ComputeInverseAii(matMixCSR const&, unsigned int, symMat3*)   @ 0x3684
Marvelous::ConjugateGradient_CPU(…, symMat3 const*, unsigned int …)      @ 0x3748
Marvelous::ConjugateGradient_CPU(…, symMat3 const*, unsigned int, …)     @ 0x3780
Marvelous::ComputeAD_CPU(matMixCSR const&, vec3 const*, unsigned int, vec3*) @ 0x3c44
Marvelous::ComputeAD_CPU(matMixCSR const&, vec3 const*, vec3*)           @ 0x3c98
```

Two external dependencies are resolved from `libCloMath.dylib`
(**Observed**, same evidence): `dot(vec3 const*, vec3 const*, unsigned int)`
and `invert(symMat3&, symMat3 const&)`. OpenMP enters through
`__kmpc_fork_call`, `__kmpc_for_static_init_4`, `__kmpc_reduce_nowait`,
`__kmpc_end_reduce_nowait` and `__kmpc_for_static_fini`.

**Derived:** the solver core is a block-sparse symmetric system solved by
conjugate gradient with a preconditioner built from `symMat3` blocks, and the
parallel entry points are OpenMP-dispatched drivers around serial kernels.

### 4.2 `matMixCSR` memory shape (**Observed**, `ev_ac2b9b3f…`, `0x34d4`)

`ComputeAD` clears `n * 3` floats, then walks rows using:

| Field | Access in pseudocode | Interpretation |
|---|---|---|
| row pointers | `*(long *)(A + 0x10)` indexed `i`, `i+1` | int32 row offsets |
| column indices | `*(long *)(A + 0x40) + col * 4` | int32 per stored block |
| **diagonal** blocks | `*(long *)(A + 0x70) + col * 0x18` | one **6-float** block per node, stride 24 B |
| **off-diagonal** blocks | `*(long *)(A + 0x88) + col * 0x24` | one **9-float** block per entry, stride 36 B |

The two separate block arrays are the observable meaning of *mixed* CSR: the
nodal diagonal is stored once in six floats, off-diagonal entries keep a full
nine.

The six diagonal floats are read at offsets `(0,0) (0,1) (0,2) (1,1) (1,2) (2,2)`
and contracted as a symmetric matrix. The nine off-diagonal floats are indexed
so that row `0` uses floats `0, 3, 6`, i.e. the full block is stored
column-major. (**Observed**; the earlier pass described the diagonal as
"lower-triangular `(m00, m10, m20, m11, m21, m22)`" — same symmetric content,
different labelling of the offset order.)

### 4.3 `ComputeInverseAii` = the preconditioner (**Observed**, `0x3684`)

The function walks the same structure and, for the entry on each row whose
column index equals the row index, calls `invert(symMat3&, symMat3 const&)` on
the diagonal block at `+0x70`.

**Derived:** the preconditioner `M` is exactly the inverted nodal diagonal, i.e.
**3x3 block-Jacobi** on the assembled operator.

### 4.4 `CG` iteration (**Observed**, `0x2848`)

Reconstructed control flow:

```
r    = b - A x                      (via ComputeAD into a scratch vector)
rho  = r . r
eps  = max(1e-15, rho * 1e-15)
while iter < maxIter and eps <= rho:
    Ap    = A p                     (via ComputeAD)
    alpha = rho / (p . Ap)
    x    += alpha p
    r    -= alpha Ap
    z     = M^-1 r                  (nodal diagonal solve)
    rho'  = r . z
    beta  = rho' / rho
    p     = z + beta p
    rho   = rho'
```

**Derived:** this is textbook Preconditioned Conjugate Gradient with a nodal
block-Jacobi preconditioner. `CG` itself is a serial driver whose inner loops are
hand-vectorised (the decompiler shows packed SIMD helper calls such as
`NEON_ext`); the OpenMP parallelism lives in `ComputeAD_CPU` and
`ConjugateGradient_CPU`.

**Unknown (deliberately not reproduced):** the exact pre-loop initialisation of
`p` and `z` is only partially visible, because the OpenMP drivers prime the
vectors before `CG` is entered. The cleanroom solver below therefore uses an
unambiguous relative residual tolerance on the residual norm instead of the
shipped `1e-15`-scaled absolute epsilon.

## 5. Cleanroom implementation in ClothCraft

New module [`src/blocksolve/`](../src/blocksolve/mod.rs), pure CPU `f64`, no
`eframe`/`wgpu` dependency, so it is part of the `--no-default-features` build.

| Recovered observation | ClothCraft implementation |
|---|---|
| 3x3 nodal blocks, symmetric diagonal stored once | `blocksolve::SymMat3` (upper triangle, six `f64`) |
| separate diagonal array + off-diagonal array over shared row pointers | `blocksolve::MatMixCsr` (own layout: strict-upper triangle only, since the assembled operator is symmetric — halves off-diagonal storage) |
| `ComputeAD` block SpMV | `MatMixCsr::spmv` |
| `ComputeInverseAii` | `MatMixCsr::block_diagonal_inverse` |
| `CG` with nodal block-Jacobi | `blocksolve::block_jacobi_pcg` |
| Dirichlet handling of the matrix-free reference | `MatMixCsr::apply_dirichlet` |

The algorithm is written from the published PCG method. The storage layout,
type names, error model and convergence reporting are this project's own
choices; none of them is copied from the artifact.

### 5.1 Integration

`System` in [`src/solver.rs`](../src/solver.rs) previously carried only a
matrix-free element Hessian path with a **point-Jacobi** preconditioner, i.e.
one scalar per dof, which cannot see the coupling between the three
translational dofs sharing a vertex. It now:

* assembles the same operator as a `MatMixCsr` (`System::assemble_operator`),
  including the mass-plus-shift diagonal and the Dirichlet convention;
* solves with block-Jacobi PCG (`System::pcg`);
* keeps the historical matrix-free path as `System::pcg_point_jacobi`, used as
  the fallback when a nodal block cannot be inverted, so one degenerate element
  cannot abort a step the old path could still take.

Two tests guard the change:

* `solver::tests::blocksolve_assembly_matches_the_matrix_free_operator` — the
  assembled `spmv` must equal the matrix-free `apply` within `1e-9` relative, at
  three regularisation shifts, with pinned dofs acting exactly as identity rows.
  This is what keeps the assembly from drifting away from the physics.
* `solver::tests::block_jacobi_bounds_point_jacobi_iterations` — the block path
  must never need more iterations than point-Jacobi on the same operator.

## 6. Measurements

Iteration counts for one Newton direction at `shift = 0`, tolerance `1e-5`
relative to the initial residual, budget 250. Reproduced with a temporary probe
that called both `System::pcg` and `System::pcg_point_jacobi` on the same
assembled system (`cargo test --lib -- --nocapture`), and now pinned as the
bound test above.

| Fixture | dofs | median nodal off-diag/diag | dt | block-Jacobi | point-Jacobi |
|---|---|---|---|---|---|
| `two_panel_demo` | 180 | 0.242 | 1/60 | **52** | 64 |
| `two_panel_demo` | 180 | 0.244 | 0.02 | **59** | 71 |
| `two_panel_demo` | 180 | 0.249 | 0.1 | **105** | 158 |
| T-shirt, 2 subdivisions | 7,362 | 0.025 | 1/60 | 36 | 36 |
| T-shirt, 2 subdivisions | 7,362 | 0.025 | 0.02 | 74 | 74 |
| T-shirt, 2 subdivisions | 7,362 | 0.025 | 0.1 | 250 (capped) | 250 (capped) |
| T-shirt, 3 subdivisions | 28,704 | 0.025 | 1/60 | 125 | 125 |
| T-shirt, 3 subdivisions | 28,704 | 0.025 | 0.02 | 151 | 151 |
| T-shirt, 3 subdivisions | 28,704 | 0.025 | 0.1 | 250 (capped) | 250 (capped) |

**Observed conclusion, including the negative result:** the assembled
block-Jacobi preconditioner is never worse and is 19–34 % better on the small
fixture whose nodal blocks carry real off-diagonal coupling. On the T-shirt
fixtures it makes **no difference at all**, and the measured reason is in the
table: their nodal blocks are roughly ten times closer to diagonal
(0.025 versus 0.24), so a scalar and a 3x3 preconditioner apply nearly the same
scaling. The change is therefore justified by structural fidelity to the
recovered operator and by the assembled-operator foundation it creates — **not**
by a claimed blanket speed-up. No timing benchmark is claimed here; only
iteration counts were measured.

**Inferred (unverified):** the T-shirt nodal blocks are nearly diagonal because
their per-node inertia term dominates the per-node stiffness diagonal at these
time steps. This explanation was not tested directly.

## 7. Corrections to the earlier pass

| Earlier pass | This pass (Observed) |
|---|---|
| `Marvelous::CGE` | **`Marvelous::CG`** — the mangling `__ZN9Marvelous2CGE…` is a *two*-character identifier `CG` followed by the nested-name terminator |
| `Marvelous::ConjugateGradient_CPUE` | **`Marvelous::ConjugateGradient_CPU`**, with two overloads (`0x3748`, `0x3780`) |
| diagonal stored as "lower-triangular `(m00, m10, m20, m11, m21, m22)`" | offsets observed at `(0,0) (0,1) (0,2) (1,1) (1,2) (2,2)`; off-diagonal stored column-major |
| class frequencies `PatternEditor 570`, `MVProperty 438`, `Fabric 343`, `Pattern 295` labelled as recovered from the main binary | reproducible only over **all** symbols including undefined imports. Over the **defined** inventory the same names occur `57`, `397`, `53`, `41` times. Both are correct for their own denominator; the table's denominator was unstated. |

Independently reproduced evidence IDs from the earlier pass, on the same
artifacts:

* `ev_83b737cf50fb67a98813d39ab9ec8a0fba899a617ab6075dced8bd2469959dd3`
  (`binary_overview`) — matches the earlier pass's `ev_83b737cf…`.
* `ev_0b63d6e5ad67f1eb2a4ec71316bca2c73c4dd9de5173f10b113d7164deda6435`
  (`list_procedures`) — matches the earlier pass's `ev_0b63d6e5…`.

The earlier pass's `batch_decompile` ID (`ev_59e11902…`) did not reproduce,
because that ID commits to the exact address set and workflow revision that were
requested. The equivalent request in this pass returned `ev_ac2b9b3f…`.

## 8. Application surface (**Observed**, direct artifact inspection)

Namespace counts over the 37,211 defined symbols of the main executable's arm64
slice: `Marvelous::` 11,457, `CloUICommon::` 5,193, `QtPrivate::` 1,750,
`QMetaObject::` 687, `View2DActionSlots::` 407, `DialogCloAvatarEditor::` 351,
`View3DActionSlots::` 350, `AnimationUtility::` 312, `MainMenuActionSlots::` 281.

Link-time dependencies include Qt **6.10.3** (`QtWidgets`, `QtQuick`, `QtQml`,
`QtWebEngine*`, `QtOpenGL*`, `QtMultimedia`, `QtSvg`, `QtXml`, `QtNetwork`,
`QtPrintSupport`, `QtPositioning`), plus `libCloScene`, `libCloMath`,
`libCloCore`, `libCloAutoRetopology`, `libCLOAPIInterface`, OpenCV 4.11,
`libPDFNetC`, `libfreeimage`, `libcurl`, `libhasp_darwin_94691`,
`Breakpad`, `Accelerate` and `OpenGL`.

Asset and project format strings present in the executable's string table:

```
$Project (ZPRJ) / Garment (ZPAC) File
ZPAC (Zipped File including PAC and Texture Images)
:Files exceeding 2GB cannot be saved as a single ZPRJ file.
Colorway Compatibility for 7.3 and 2024.0 (ZPRJ)
3D File (*.avt *.fbx *.glb *.gltf *.obj)
AVT File (*.avt)          AVTP File (*.avtp)          %1.zprj
&The Fabric (ZFAB) file has been saved.
```

Automation surface: a mangled symbol
`HeadlessApiController::ExecutePython` is present, alongside `QPythonCompleter`,
`QPythonHighlighter`, `:/languages/python.xml`, `CLOAPI`,
`Marvelous::IGuiApiCallback` and `Load Python Script File`.

**Derived:** the shipped application exposes a headless Python automation
controller. That is the lowest-risk interoperability route for a user's own
data, and it is distinct from reimplementing the engine.

## 9. What was not done

* **The `.zprj` schema.** The extension, the 2 GB single-file cap, the
  colourway-compatibility branch and a Python API are observed. The *key
  vocabulary* of the serialized tree is not. ClothCraft does not parse these
  formats and does not pretend to; see [interoperability.md](interoperability.md).
* **The EBP, WRAPD and RootParity solver variants.** The libraries exist
  (`libCloSimulateEBP`, `libCloSimulateWRAPD`, `libCloSimulateRootParity`); their
  formulations were not decompiled in this pass.
* **GPU paths.** `libCloSimulateCuda` and `libCloSimulateMetal` were not
  examined.
* **Fabric preset numerics.** Not extracted, and deliberately not copied: pre-set
  libraries are proprietary asset content.
* **Main-executable decompilation.** The architecture here comes from the symbol
  table, load commands and strings of the shipped binary, not from a Ghidra
  decompilation of the 39.7 MB `__text`. Ghidra was used where it was
  proportionate: the 35-procedure solver core.

## 10. Reproduction

```bash
# Thin slices (Ghidra rejects universal Mach-O)
FW=/Applications/CLO.app/Contents/Frameworks
lipo -thin arm64 "$FW/libCloSimulateSolvers.dylib" -output clo3d_target/libCloSimulateSolvers.arm64.dylib
shasum -a 256 clo3d_target/libCloSimulateSolvers.arm64.dylib
# 3050227736cfcd66db262f552660551e735909900e1f8632a301b7c564143446

# Symbol structure without a disassembler
B=clo3d_target/CLO_Standalone_OnlineAuth_arm64
nm -a "$B" | wc -l                                   # 50164
nm "$B" | awk '$2=="T"||$2=="t"' | wc -l             # 32792
otool -L "$B" | grep -iE 'Qt6|libClo|python'

# Ghidra route (REA)
#   open_binary clo3d_target/libCloSimulateSolvers.arm64.dylib
#   binary_overview / list_procedures
#   batch_decompile 0x2848 0x34d4 0x3684 0x3748 0x3780 0x3c44 0x3c98

# Verification of the cleanroom implementation
cargo test --locked --no-default-features -j 2 --lib
```

## 11. License and provenance

The CLO artifacts analysed here are **not** distributed with this repository.
The earlier-pass documents preserved beside this file are prior analysis of the
same artifacts and contain no CLO code. All Rust in `src/blocksolve/` and the
integration in `src/solver.rs` is original work under this repository's MIT
licence, written from the published Preconditioned Conjugate Gradient method and
from the structural observations recorded above.
