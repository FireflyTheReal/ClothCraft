# CLO3D fabric material model — reverse-engineering evidence

**Status:** evidence base for a future *calibrated materials* roadmap item. No
implementation is proposed here and no code changed.

This document records what was read out of the shipped CLO 2026.1.310 libraries
with Ghidra 12.1.4 through REA. It is **not** a transcription of CLO code: no
decompiled body, asset, preset or table is reproduced. The short pseudocode
fragments below are structural summaries written to explain a mechanic, and the
only numeric literals kept are the algorithm constants needed to describe that
mechanic (rate limits, clamps, floor factors). **No fabric preset value,
material table, `.zfab`/`.zprj` content or per-fabric number appears in this
repository.**

## 0. Evidence discipline

Every claim is tagged:

* **Observed** — read directly from the shipped artifact (symbol table, string
  table, decompiler output).
* **Derived** — a conclusion that follows from observations.
* **Inferred** — a plausible explanation that has *not* been verified.
* **Unknown** — not established, and not guessed.

REA Evidence IDs are deterministic record identifiers: the same operation
against the same artifact and provider profile yields the same ID. Every ID
quoted below is verbatim from this session.

## 1. Targets

CLO bundle version is **2026.1.310** (`CFBundleShortVersionString`). Ghidra
rejects universal Mach-O, so each fat library was thinned with `lipo -thin arm64`
first. Slice identity is what REA bound:

| Source (fat, read-only) | Thin `arm64` slice analysed | Slice bytes | Slice SHA-256 |
|---|---|---|---|
| `Contents/Frameworks/libCloSimulatePhysics.dylib` | `clo3d_target/libCloSimulatePhysics.arm64.dylib` | 97,632 | `e6fd6f889b396b976810cc4267fd2b6823e99ec3767a1a781d5b91ce55d2ede1` |
| `Contents/Frameworks/libCloSimulateDataStructures.dylib` | `clo3d_target/libCloSimulateDataStructures.arm64.dylib` | 76,112 | `f7582de56fa00d36922d814c6ab83219a450d4d3496bb6ebd6ae389ee146c37c` |
| `Contents/Frameworks/libCloDeforMarvel.dylib` | `/tmp/lcdm.arm64.dylib` (outside the repo) | 905,824 | `c6c409c183c8f2d5a428a9efdd35ada1f9ee4a99d7018560ce8bd25b52df0f05` |
| `Contents/Frameworks/libCloSimulateSolvers.dylib` | `clo3d_target/libCloSimulateSolvers.arm64.dylib` (recreated, not re-analysed) | 69,888 | `3050227736cfcd66db262f552660551e735909900e1f8632a301b7c564143446` |

The solver slice hash reproduced **byte-for-byte** the value recorded in
[clo3d-reverse-engineering.md](clo3d-reverse-engineering.md) (**Observed**), so
the artifacts analysed in that pass and this one are the same builds.

`libCloDeforMarvel` was not in the original target list. It was added because it
is where the nonlinear stiffness is actually **consumed and evaluated**; without
it the material model could only be described, not explained. Its fat source
hash is `9f3ab89f838578de1f2080d56e6f24fad69a65ce61d52b581c6e7458f6de7e4f`.

All three libraries are **not stripped**, so structure is recoverable from names
alone (**Observed**, direct `nm`; 76 / 63 / 901 defined text symbols
respectively). Ghidra was used on the small enumerable set of functions that
carry the model, never for whole-program decompilation.

## 2. Evidence index

All IDs below were produced in this session.

| # | Operation | Evidence ID | What it establishes |
|---|---|---|---|
| 1 | `binary_overview` (physics) | `ev_e7e63103c6c665823bf40703a6b6e83d893e6a7b9393845ad9f3a75b42719836` | 13 segments, **186 procedures**, 393 strings, `image_base 0x0` |
| 2 | `list_procedures` (physics) | `ev_4bc07c3497735fdc23565788d24f4ee3560dc0bb125e3c7208932af5a54b923e` | full procedure inventory and addresses; externals resolve to `libCloMath` |
| 3 | `batch_decompile` (physics) | `ev_40a27049ed5dd35ca925bdddde3406c090154b7bf1a7372ee9d8f9f7642772b0` | `NonlinearStiffness::Save`/`Load`, `To`/`FromNonlinearStiffnessPiecewiseFn`, `DeforMarvelSpring::Set`/`Load` |
| 4 | `batch_decompile` (physics) | `ev_e74d8b84ba16f2ef8e068c55356aee6b9d4499d5510d8d5c6748510dc9bff3f0` | `PhysicalProperty::InitializePhysicalProperty`, `operator==`, `Copy`, `InterpolatePP`, `ApplyMinConstraint` |
| 5 | `batch_decompile` (physics) | `ev_eec828bd6c41f281bd2086a296bb55804c9f79b3d14a352c2c8ee1b75a7941ae` | `PhysicalProperty::Save` — complete key↔offset map |
| 6 | `batch_decompile` (physics) | `ev_cc7a9c55c30e81f0e852e463a99e719d4c754a4ce82e5bbe217490245354bfdd` | `PhysicalProperty::Load` — legacy/v2 precedence and unit migration |
| 7 | `batch_decompile` (physics) | `ev_4dbf17b03153f6adee286982f0664f92d43607f94b1d12e219fc2c86f08cf67c` | `WindProperty::Initialize`, `UpdateMatrix`, `GetCurrentStrengthPortion`, `GetMatrixFromPosRot` |
| 8 | `batch_decompile` (physics) | `ev_8654aae0bfdc672cd0be0b0e93901bb42d5539b66f99e3d464fbd4659d0c0784` | `WindProperty::Save` — complete key↔offset map |
| 9 | `binary_overview` (datastructures) | `ev_c539a25166b6b3f57a9b84c15d7ae1dd2c352400060281ae88ee2c05ee2ace41` | 13 segments, **113 procedures**, 264 strings |
| 10 | `batch_decompile` (datastructures) | `ev_2840fda7bb0ffcd552590d87021a561b430adf4ae4efb5a22a3afa272405ff31` | `HashTable::GetEdgeToVertexIndex`, `KDTree::construct`, `KDTreeInternalNode::Search`, `KDTree::getSquaredDistanceToBox` |
| 11 | `batch_decompile` (deformarvel) | `ev_6aeec3a603c58a8fcabb58b31cfcf76a23f937993168544c1dd46dede2b8773f` | `DeforMarvelTriangle::GetLayerNonlinearStiffness`, `GetLayerStiffness`, `GetNonlinearStiffnessFactor` |
| 12 | `batch_decompile` (deformarvel) | `ev_adcd5aee43104e84384bc932c558adbe35aa316f05b29a7106343a5ef5da9f07` | `DeforMarvelTriangle::GetRawLayerNonlinearStiffness` — the knot evaluator |

Symbol structure (section 2) also comes from direct `nm` / `strings` on the
shipped slices, which needs no disassembler and yields no Evidence ID.

## 3. How nonlinear stiffness is represented

**Observed** (`nm` on the physics slice; `ev_40a27049…`):

```
Marvelous::NonlinearStiffness::Save(MVVariantMap&)
Marvelous::NonlinearStiffness::Load(MVVariantMap const&)
Marvelous::ToNonlinearStiffnessPiecewiseFn(vector<NonlinearStiffness> const&)
Marvelous::FromNonlinearStiffnessPiecewiseFn(clo::NonlinearStiffnessPiecewiseFn<float> const&)
```

**Observed** (`ev_40a27049…`): `NonlinearStiffness` is a 16-byte polymorphic
record — a vtable slot at `+0`, then two floats:

* `fLengthRatio` at `+8`
* `fStiffnessRatio` at `+0xc`

`Save` writes exactly those two keys into a child `MVVariantMap` (`fLengthRatio`,
`fStiffnessRatio`); `Load` reads exactly those two back. So a "nonlinear
stiffness" is **one sample point on a stiffness-multiplier-versus-stretch
curve**, not a function object.

**Observed** (`ev_40a27049…`): `clo::NonlinearStiffnessPiecewiseFn<float>` is a
**fixed 32-byte record**: four length ratios in the first four floats, four
stiffness ratios in the next four. Both converters confirm this:

* `To…` walks the input vector, **caps the knot count at four**
  (`if (3 < n) n = 4;`), and writes each knot as (length, stiffness) pairs.
* `From…` walks four fixed slots and **keeps only slots whose length ratio is
  greater than zero**, so an unused slot is marked by a zero length ratio.

**Derived:** the serialized fabric model can express an arbitrary-length knot
vector, but the runtime/solver-facing form is a four-knot piecewise function.
More than four knots are silently truncated by `To…`.

**Observed** (`strings` on the physics slice; key names confirmed against
`ev_eec828bd…` / `ev_cc7a9c55…`): a `PhysicalProperty` carries **four** such
curves, one per in-plane/warp, in-plane/weft, bending and left-shear channel
(`listNonlinearSuK`, `listNonlinearSvK`, `listNonlinearHK`,
`listNonlinearLeftShearK`).

The two directions of the piecewise-function type are used by different
libraries (**Observed**, `nm -u` across `Contents/Frameworks`):
`libCloScene` **calls** `ToNonlinearStiffnessPiecewiseFn` (the UI/serialisation
side converts a knot vector into the fixed form) and `libCloDeforMarvel`
**calls** `FromNonlinearStiffnessPiecewiseFn` (the solver side converts it back
into a knot vector). **Derived:** the fixed four-slot type is the boundary
object between the authoring/serialisation layer and the solver layer.

## 4. How nonlinear stiffness is evaluated

The evaluator lives in `libCloDeforMarvel.dylib`, not in the physics library.
Four functions carry the mechanic (**Observed**, `ev_6aeec3a6…`,
`ev_adcd5aee…`).

### 4.1 The knot evaluator

`DeforMarvelTriangle::GetRawLayerNonlinearStiffness(unsigned direction, float stretch)`
takes a direction index (0 = warp, 1 = weft) and a stretch ratio, and returns an
effective stiffness. Reconstructed structure:

```
base      = physicalProperty[direction == 0 ? +0xc : +0x10]   // fSuK / fSvK
if (physicalProperty[+0xe0] == 1) {                           // bUseExpApproximation
    rate   = physicalProperty[direction == 0 ? +0x80 : +0x84] // fexpSuK / fexpSvK
    mult   = exp((max(1, stretch) - 1) * rate)
    result = max(1.0, base * mult)                            // floor 1.0
} else {
    knots  = physicalProperty[direction == 0 ? vector@+0x20 : vector@+0x38]
    if (knots is empty) {
        if      (stretch <= 1.0)  mult = 1.0
        else if (stretch <= 1.1)  mult = lerp(1.0, 20.0, (stretch-1)/0.1)
        else                      mult = 20.0
    } else {
        mult = piecewiseLinear(knots, stretch)   // clamped outside the knot range
    }
    result = base * mult
    if (firstKnot.stiffnessRatio < 0.01 && result < 3000.0) result = 3000.0
}

```

**Observed consequences:**

* Two mutually exclusive representations of the same idea: an explicit
  multiplier-versus-stretch **knot curve**, or a **closed-form exponential**
  `exp(rate × (stretch − 1))`. The switch is the serialized boolean
  `bUseExpApproximation` at `+0xe0`.
* In the exponential form the serialized `fexp*K` values are **rate
  coefficients**, not stiffnesses: they multiply `(stretch − 1)` inside the
  exponential (**Derived**, from their position in the expression).
* Both forms are floored at a multiplier of `1.0`, so a nonlinearity can only
  ever *stiffen*, never soften below the base stiffness.
* The multiplier is capped at **20×** in the three-term default (empty knot
  curve) path.
* Piecewise interpolation clamps outside the knot range: below the first knot it
  returns the first knot's multiplier, above the last knot the last knot's
  multiplier (**Observed**, the search loop stops at the final element rather
  than extrapolating).
* There is an absolute stiffness floor of `3000.0` that only fires when the
  first knot's multiplier is near zero (a "zero-stiffness fabric" guard). It is
  **not** applied on the exponential path (**Observed**; an asymmetry).
* The empirical `lerp(1 → 20)` over the first 10 % of stretch is the behaviour
  when *no* knot curve is authored at all: a fabric with an empty curve still
  stiffens twentyfold by 10 % extension. **Derived:** this is the implicit
  default that the UI's piecwise form makes explicit.
* The knot storage is `(lengthRatio, stiffnessRatio)` with the *stiffness* value
  being a **multiplier on the base `fSuK`/`fSvK`**, not an absolute stiffness
  (**Observed**; the evaluator multiplies `base * mult` in both paths).
  **Inferred:** a `stiffnessRatio` of 1 therefore means "linear".

### 4.2 The rate-limited application

Neither of the two entry points returns a stiffness that is applied directly.
Both write into a caller-supplied accumulator and clamp the *change* per call
(**Observed**):

```
target = <evaluated stiffness>
delta  = target - *current
delta  = clamp(delta, -2000.0, +2000.0)
*current += delta
```

The same ±2000 per-call limit appears in `GetLayerNonlinearStiffness` (the
exponential form) and in `GetLayerStiffness` (the knot form).

**Derived:** the shipped engine **slews** the applied stiffness toward the
nonlinear target rather than jumping to it, so crossing a knot does not inject a
stiffness discontinuity into the timestep. This is a numerical-stability
mechanism, not a material property.

`GetLayerNonlinearStiffness(a, b, c, applyFlag, float& out)` is the exponential
variant in isolation:
`out ← clamp(max(1.0, a·exp(max(1,c) − 1)·b), out − 2000, out + 2000)`, written
only when `applyFlag` is set.

### 4.3 The aggregate nonlinear factor

`DeforMarvelTriangle::GetNonlinearStiffnessFactor(float stretchU, float stretchV)`
returns a single scalar factor (**Observed**, `ev_6aeec3a6…`):

```
if (physicalProperty[+0xe1] & 1) return 1.0            // bEband disables it entirely
baseU/baseV = bUseExpApproximation ? [+0x90, +0x94] : [+0xc, +0x10]
if (baseU + baseV < 1e-6) return 1.0
rawU = GetRawLayerNonlinearStiffness(U, stretchU)   // only if stretchU > 1
rawV = GetRawLayerNonlinearStiffness(V, stretchV)   // only if stretchV > 1
return clamp((rawU + rawV) / (baseU + baseV), 1.0, 20.0)
```

**Observed:** the factor is the *ratio of the current directional stiffnesses to
a baseline*, averaged over the two in-plane directions, clamped to `[1, 20]`.
The baseline for the exponential path is a **four-float block at `+0x90…+0x9c`
that `Save` and `Load` never touch at all** — see §5.2.

**Observed:** the boolean `bEband` (`+0xe1`) switches the whole nonlinear factor
off, returning exactly 1.0. **Inferred:** this is the "E-band" / membrane-only
material mode.

## 5. What `PhysicalProperty` Load/Save serialises

`PhysicalProperty` is a polymorphic ~232-byte object with a vtable pointer
(**Observed**, `ev_e74d8b84…`; `InterpolatePP` stores a vtable into the result).

### 5.1 Serialization guard

**Observed** (`ev_eec828bd…`): `Save(map, bool force)` writes nothing and
returns 0 unless *either* the caller forces it *or* the object's byte at `+0x08`
is exactly 1. `Load`, `Copy` and `InterpolatePP` all set that byte to 1;
`InitializePhysicalProperty` sets it to 0.

**Derived:** `+0x08` is an "initialised / modified" flag, and saving a
freshly default-constructed, never-touched property is a no-op that reports
`false`. Saving a loaded or copied one succeeds.

### 5.2 Recovered key ↔ offset map

**Observed** (`ev_eec828bd…` for the write side, `ev_cc7a9c55…` for the read
side). Both functions agree on every offset. Offsets are byte offsets from the
object base.

| Slot | Serialized key | Notes (all **Observed**) |
|---|---|---|
| `+0x0c` | `fSuK` | in-plane warp (structural U) base stiffness |
| `+0x10` | `fSvK` | in-plane weft (structural V) base stiffness |
| `+0x14` | `fHK` / `fRightShearK_v2` | written **halved** under the legacy key, raw under `_v2` |
| `+0x18` | `fLeftShearK` / `fLeftShearK_v2` | written **halved** under the legacy key, raw under `_v2` |
| `+0x20` | `listNonlinearSuK` | `vector<NonlinearStiffness>` |
| `+0x38` | `listNonlinearSvK` | `vector<NonlinearStiffness>` |
| `+0x50` | `listNonlinearHK` | `vector<NonlinearStiffness>` |
| `+0x68` | `listNonlinearLeftShearK` | `vector<NonlinearStiffness>` |
| `+0x80` | `fexpSuK` | exponential rate, warp |
| `+0x84` | `fexpSvK` | exponential rate, weft |
| `+0x88` | `fexpRightShearK` | exponential rate, right shear |
| `+0x8c` | `fexpLeftShearK` | exponential rate, left shear |
| `+0x90, +0x94` | *(none)* | **not serialized**; baseline stiffness pair for the exp path |
| `+0x98, +0x9c` | *(none)* | **not serialized**; mirror of `+0x14`/`+0x18` |
| `+0xa0` | `fBuK` / `fBuK_v2` | legacy key written **divided by π**, `_v2` raw |
| `+0xa4` | `fBvK` / `fBvK_v2` | legacy ÷π, `_v2` raw |
| `+0xa8` | `fBhK` / `fBRightShearK_v2` | legacy ÷π, `_v2` raw |
| `+0xac` | `fBLeftShearK` / `fBLeftShearK_v2` | legacy ÷π, `_v2` raw |
| `+0xb0` | `fBuLR` | "LR" variant; no dilution factor |
| `+0xb4` | `fBvLR` | |
| `+0xb8` | `fBhLR` | |
| `+0xbc` | `fBLeftShearLR` | |
| `+0xc0` | `fBucklingStiffnessU` | buckling group |
| `+0xc4` | `fBucklingStiffnessV` | |
| `+0xc8` | `fBucklingStiffnessH` | |
| `+0xcc` | `fBucklingStiffnessLeftShear` | |
| `+0xd0` | `fIDS` | meaning not established — see §10 |
| `+0xd4` | `fFriction` | |
| `+0xd8` | `fDensity` | also the driver of the minimum-constraint floor (§6.1) |
| `+0xdc` | `fThickness` | |
| `+0xe0` | `bUseExpApproximation` | bool — selects the exponential stiffness form |
| `+0xe1` | `bEband` | bool — disables the nonlinear stiffness factor |
| `+0xe4` | *(none)* | present in the object, not serialized by name |

**Observed — schema evolution.** Both legacy and `_v2` keys are always written by
`Save`. On `Load` the legacy keys are read **first**, derive/scale intermediate
results, and then the `_v2` keys are read and **override only if present**
(the variant lookup leaves the destination untouched and returns 0 for a missing
key). Concretely, when only legacy keys exist:

* `fHK` is read into `+0x14` and then **doubled in place**; `fLeftShearK` is read
  into `+0x18`, and if absent it is copied from the already-doubled `+0x14`
  value. So the legacy `0.5` write factor is exactly undone on read.
* `fBhK` defaults to the mean of `fBuK`/`fBvK` when absent; `fBLeftShearK`
  defaults to `fBhK`. Then **all four** `fB*` values are multiplied by π,
  undoing the legacy `÷π` write. The `_v2` keys then overwrite with raw values.
* `fBhLR` defaults to the mean of `fBuLR`/`fBvLR`; `fBLeftShearLR` defaults to
  `fBhLR`.
* `fBucklingStiffnessH` defaults to the mean of U and V;
  `fBucklingStiffnessLeftShear` defaults to H.
* `listNonlinearLeftShearK` defaults to a **copy of** `listNonlinearHK` when the
  key is absent.

**Derived:** the same four `+0x14/+0x18` and `+0xa0…+0xac` memory slots changed
*meaning* between schema versions — they are "bending" (`fHK`, `fBhK`) in the
legacy vocabulary and "right shear" (`fRightShearK_v2`, `fBRightShearK_v2`) in
the current one, with a factor-of-two (in-plane) and a factor-of-π (bending)
unit change between the two readings. A reader that mixed the two key sets would
silently mis-scale the shear stiffness.

**Observed** (`ev_cc7a9c55…`): at the end of `Load`, the `+0x08` flag is set to
1 (making the object savable) and then, **unless the caller's boolean argument
is set**, the minimum-constraint pass of §6.1 runs. **Derived:** `Load`'s boolean
means "load raw / skip the minimum-constraint pass"; `Save`'s boolean means
"force the write even if unmodified".

### 5.3 The object has a non-serialized state block

**Observed:** `+0x90…+0x9c` (four floats) is read by
`GetNonlinearStiffnessFactor`, written by `InitializePhysicalProperty`, copied by
`Copy`, compared by `operator==` and interpolated by `InterpolatePP` — and **is
never written or read by `Save`/`Load`**. `InitializePhysicalProperty` gives it
the same values as `+0x0c…+0x18`.

**Derived:** it is the **pre-stretch baseline copy** of the in-plane and
shear/bending stiffnesses, used as the denominator of the nonlinear ratio in
§4.3. It is runtime state reconstructed on load, not fabric data — so a faithful
cleanroom reader must know to rebuild it (from `+0x0c…+0x18`) rather than expect
it in the file.

## 6. Minimum constraint and interpolation

### 6.1 `ApplyMinConstraint` (**Observed**, `ev_e74d8b84…`)

```
d = fDensity (+0xd8)
if (d < 1e-5) d = 1e-5
hiFloor = d * 3.0 * 1e6
loFloor = d * 0.2 * 1e6
if (all four of +0x0c..+0x18 < hiFloor) set all four = hiFloor
if (all four of +0xa0..+0xac < loFloor) set all four = loFloor
```

**Derived:** the minimum constraint is **all-or-nothing per group**. If *any one*
of the four in-plane stiffnesses is at or above the density-derived floor, none
are raised; the floor only fires when the whole group is degenerate. The same
group semantics apply to the four bending/`fB*` values with a floor twenty
times smaller. **Derived:** density is used as a scale proxy for a
physically-sane stiffness, which is why a fabric's mass and its stiffness floor
are not independent.

### 6.2 `InterpolatePP(first, second, t)` (**Observed**, `ev_e74d8b84…`)

**Observed:** the result is first initialised with the same default constants as
`InitializePhysicalProperty`, then:

* every scalar/float slot is lerped `value = second·t + first·(1−t)`, with exact
  copies at `t = 0` and `t = 1`;
* the four nonlinear knot vectors are **copied verbatim from one input**, not
  interpolated (`FUN_00003b7c` vector copies from one source only);
* the two booleans at `+0xe0`/`+0xe1` are taken from one input;
* the `+0x08` flag is set to 1.

**Derived:** blending two fabrics interpolates the *scalar* constants but
**picks** one fabric's nonlinear curves. A blended material therefore does not
get a blended nonlinearity. **Unknown:** which input's curves win — the
decompiler's argument order for this function is not reliable enough to say.

## 7. The wind property

**Observed** (`ev_4dbf17b03153f6adee286982f0664f92d43607f94b1d12e219fc2c86f08cf67c`,
`ev_8654aae0…`). `WindProperty` exposes the following state (offset ↔ key, write
side confirmed against the read side's public accessors):

| Offset | Key | Public accessor(s) |
|---|---|---|
| `+0x08` | `Activate` | `GetActivate` / `SetActivate` |
| `+0x09` | `WindType` | `GetWindType` / `SetWindType` |
| `+0x0a` | `IsUnlimitedBound` | `IsUnlimitedBound` / `SetIsUnlimitedBound` |
| `+0x0c,0x10,0x14` | `TranslateX/Y/Z` | `SetPosition(float,float,float)`, `TranslateInWorld` |
| `+0x18,0x1c,0x20` | `RotationX/Y/Z` | `SetRotation`, `Rotate` |
| `+0x24` | `Strength` | `GetStrength` / `SetStrength` |
| `+0x28` | `Decay` | `GetDecay` / `SetDecay` |
| `+0x2c` | `Frequency` | `GetFrequency` / `SetFrequency` |
| `+0x30` | `Turbulence` | `GetTurbulence` / `SetTurbulence` |
| `+0x34` | *(none)* | `GetCurrentStrengthPortion` — current animated strength portion |
| `+0x38` | *(none)* | `unordered_map<WindKeyActivateType,bool>` |
| `+0x60` and `+0x6c` | `BoundingSphereSize`, `BoundingBoxSizes` | a sphere size and a box size, each a `vec3` |
| `+0x78` | `Matrix` | cached local-to-world `mat4` |

Additional serialized keys that are **not** object state: `AddActivate`,
`AddStrength`, `AddbDecay`, `AddFrequency`, `AddTurbulence`, `AddTranslate`,
`AddTranslateX/Y/Z`, `AddRotate`, `AddType`. Two of them (`AddActivate`,
`AddType`) are written as the constant `true`; the rest are written as booleans
read from the wind-key map at keys 0…8.

**Observed — the wind-key activation table.** `Initialize` inserts **13** entries
into the `unordered_map` at `+0x38` (keys 0 through 12). All are `false` on
construction except the last; `SetWindKeyActivateState(int, bool)` and
`GetWindKeyActivateState(int)` address it by integer index, and `Save` reads map
entries 0…8 for the `Add*` booleans and a separate lookup for the standalone
`AddActivate`/`AddType` flags.

**Derived:** the `Add*` keys are a *per-animation-key opt-in mask* — which
authoring channels are allowed to contribute when a wind keyframe drives the
property. They are serialized flags, not wind physics.

**Observed — the local-to-world matrix.** `GetMatrixFromPosRot` builds the
transform from the position and rotation triples with an explicit
degrees→radians conversion, composing `T · Rz · Ry · Rx` (three successive
`mat4::operator*` calls, translation outermost). `UpdateMatrix` recomputes the
cached `+0x78` matrix from the current position and rotation; `Initialize` calls
it to prime the cache.

**Derived:** wind is a **transformed bounding volume with a direction, an
amplitude, a decay, a frequency and a turbulence term**, not a global vector.
The property also owns a time-varying `CurrentStrengthPortion`, so the applied
strength is an envelope times the authored strength.

## 8. The spring

**Observed** (`ev_40a27049ed5dd35ca925bdddde3406c090154b7bf1a7372ee9d8f9f7642772b0`):

```
DeforMarvelSpring::DeforMarvelSpring(uint, uint, uint, float, float, float)
DeforMarvelSpring::Set(uint, uint, uint, float, float, float)
DeforMarvelSpring::Load(FILE*, uint version)
```

`Set` stores six consecutive 32-bit words into the object: the three float
arguments and the three unsigned arguments. **Observed:** the object therefore
begins with a three-word floating-point triple and a three-word integer triple.

`Load` is **version-gated** and reads three different layouts:

| `version` | Format |
|---|---|
| `< 20` | text, 9 fields parsed as 3 ints, 3 floats, 3 ints |
| `20 … 49` | text, 6 fields (3 ints, 3 floats), optionally a further `int,float,float` when `version > 34` |
| `>= 50` | binary `fread` of six 4-byte words, then a 1-byte flag, then an `int` and a `float` |

**Observed migrations on read:**

* For `version < 15` **and** the object's first word equal to 2, the word at
  `+0x14` is scaled by 1000 (an integer-valued scale migration).
* In the binary path, when the 1-byte flag at `+0x18` is 1, the word at `+0x0c`
  is overwritten by the trailing `int`, and the word at `+0x14` is multiplied by
  the trailing `float`.

**Derived:** the spring container carries a per-format version stamp and the
loader deliberately reproduces historical unit/scale mistakes, exactly like the
`_v2` override pattern in §5.2. Stiffness and rest-length data written by an old
CLO version is passed through a scaling path, not read literally.

**Unknown:** which of the six words is stiffness, which is damping, which is
rest length, and what the three integers identify (plausibly two vertex indices
and a spring category, but this was not verified). See §10.

**Observed** (unrelated but adjacent): the library also imports `rand()`,
`cos()`, `sincosf()` and `fscanf`, and its string table contains a
`"%d %d %d %f %f %f %d %d %d"` format string, matching the `Load` layout above.

## 9. Spatial acceleration structures

Secondary to the material model, recorded for completeness.
**Observed** (`ev_2840fda7…`).

* `KDTree::construct` computes an axis-aligned bounding box over a `vec3` array
  with a 12-byte stride, storing `min` at `+0x24…+0x2c` and `max` at
  `+0x30…+0x38`, then builds a subtree and stores the root at `+0x08`.
  `getSquaredDistanceToBox` confirms exactly those six floats are the box and
  returns the standard clamped squared distance (zero inside).
* `KDTreeInternalNode::Search` is branch-and-bound: it chooses the near child by
  the sign of the query coordinate minus the split value, recurses, then prunes
  the far child when the accumulated squared distance is no better than the
  worst retained result (`FLT_MAX` when the result list is empty). The tree keeps
  a bounded, distance-sorted result list at `+0x60`, so the structure supports
  both single-nearest and k-nearest queries
  (`FindNearestNeighbor(vec3, int, vector<pair<int,float>>&)`).
* `HashTable` buckets are 24-byte (`std::vector`) slots; each element is a
  24-byte record holding a vertex pair and an edge index at `+0x10`.
  `GetEdgeToVertexIndex(u32*)` flattens the table into a two-entry-per-edge
  array indexed by edge index.
* Edge tables exist for both triangle meshes (cloth) and tetrahedron meshes
  (volumetric bodies), and `PLIndex` is an ordered `(piece, line)` pair with
  comparison operators — the identity used for pattern edges.

**Derived:** collision/broadphase is a KD-tree over points plus a hash table of
mesh edges, shared between the cloth and volumetric paths.

## 10. What could NOT be established

Stated explicitly, and not guessed:

1. **The meaning of `fIDS`** (`+0xd0`). It is a serialized float, lerped by
   `InterpolatePP`, compared by `operator==`, and surfaced nowhere else in the
   analysed libraries. No expansion of the initialism was found.
2. **The meaning of the `LR` suffix** in `fBuLR`/`fBvLR`/`fBhLR`/
   `fBLeftShearLR`. The values are not scaled by π or 0.5 on either read or
   write, so they are dimensionally different from the plain `fB*` group —
   but what "LR" denotes is Unknown.
3. **Per-field typing of `DeforMarvelSpring`.** The six words are written as
   floats and integers by `Set`, but `Load` reads and scales the same slots as
   mixed int/float across format versions. Which word is stiffness, damping and
   rest length, and what the three integers identify, is Unknown.
4. **Which unit the base stiffnesses are in.** `fSuK`/`fSvK` and the `fB*`
   groups are only ever multiplied, divided by π, floored and compared. No
   conversion to engineering units was observed. The π and 1000 factors show
   that historical units changed, but the current unit is Unknown.
5. **The shear buckling and left-shear nonlinear evaluation paths.** Only the
   two in-plane directions (warp/weft) go through
   `GetRawLayerNonlinearStiffness`. `fexpRightShearK`, `fexpLeftShearK`,
   `listNonlinearHK` and `listNonlinearLeftShearK` are serialized and read, but
   the function that evaluates them was not decompiled.
6. **Which input wins in `InterpolatePP`.** The decompiler's argument order for
   the two `PhysicalProperty const&` parameters is not reliable, so it is
   Unknown whose knot curves survive a blend.
7. **`PhysicalProperty::Load`'s behaviour for the `+0x90…+0x9c` block.** It is
   never serialized, but no code that reconstructs it on load was found either.
   Whether it is rebuilt from `+0x0c…+0x18` by a caller, or simply left at
   whatever the constructor wrote, is Unknown.
8. **The authoring-side format.** No `.zfab`, `.zprj` or preset file was opened
   or decoded, and no key *values* were extracted. This document establishes the
   in-memory and variant-map schema only.
9. **Runtime behaviour.** Nothing here was executed. Every statement is static
   analysis of the shipped binaries; the solver, the UI and the file loaders were
   not run.
10. **Reproduction of the 2026.1.310 solver Evidence IDs.** The solver slice
    hash matched the earlier pass byte-for-byte, but the solver Ghidra pass was
    not re-run in this session, so its Evidence IDs are cited from
    [clo3d-reverse-engineering.md](clo3d-reverse-engineering.md) rather than
    re-observed here.

## 11. Surprises worth flagging

* **Stiffness is slewed, not applied.** The ±2000-per-call rate limit is the
  single most transferable mechanic found, and it is invisible from the
  serialized schema alone. A cleanroom implementation that writes the evaluated
  stiffness straight into the force computation will behave differently at knot
  boundaries.
* **The nonlinearity can only stiffen.** Both representations are floored at a
  multiplier of 1.0. Modelling softening (a real effect in some textiles) is not
  expressible in this schema — the shell fabric softens only through the
  *base* stiffness.
* **The same memory slot is "bending" in one schema and "shear" in the next**,
  with a factor of 2 (in-plane) and π (bending) between the two readings. A
  reader that mixes the key sets silently mis-scales.
* **Four floats of live material state are not in the file at all**
  (`+0x90…+0x9c`), and they are the denominator of the nonlinear factor. Load
  order matters.
* **The physical default is already nonlinear.** With no knot curve authored,
  the stiffness multiplier still ramps to 20× over the first 10 % of extension.
  "Linear material" is a curve with a single knot, not an absence of curve.
* **`bEband` is a hard off switch**, not a scaling, for the nonlinear factor.
* **The piecewise form is lossy in a specific way**: the authoring side may hold
  an arbitrary number of knots, but the solver boundary object holds four, and
  the conversion truncates silently rather than failing.

## 12. What this implies for a cleanroom material model in ClothCraft

The point of this section is **mechanics to reimplement from our own measured
data**, never copied tables or presets.

Mechanics the evidence supports reimplementing independently:

1. **A material is a named set of orthogonal stiffness channels, not one
   number.** The shipped model separates in-plane warp, in-plane weft, bending
   and left shear, plus a buckling group. ClothCraft's material type should
   carry the same *axes* so that calibration data can be attached per axis, even
   if our parameterisation differs.
2. **Spread versus bending needs distinct non-dimensional channels.** The
   serialized `fDensity`, `fFriction`, `fThickness` triple alongside the
   stiffness groups shows the engine keeps the areal/geometric properties
   separate from the elastic ones. Keeping density and thickness as first-class
   material fields is what makes a density-derived stiffness floor meaningful.
3. **Nonlinearity as a multiplier on the base stiffness, over a stretch ratio.**
   This is a clean, testable formulation: `k_eff = k_base · m(stretch)`. We can
   adopt the *form* with our own measured `m`, our own units and our own
   identity, and validate it by finite differences and by checking that
   `m ≡ 1` reproduces the linear model exactly.
4. **Two interchangeable representations.** A knot table for measured data and a
   closed-form `exp(rate·(stretch−1))` for analytic fitting. Offering both, with
   an explicit switch, makes calibration (data → curve) and analysis
   (curve → closed form) both first-class. A documented, *non-silent* knot
   limit is better than the shipped silent truncation — we should reject or warn.
5. **A slug/rate limit on the applied stiffness.** This is a numerical-stability
   device, and it should be a documented solver option with a named unit, not a
   hidden constant. It is directly testable: the applied stiffness must never
   move more than the configured bound between steps.
6. **An explicit, group-wise minimum constraint driven by density.** The
   all-or-nothing group semantics are observable and easy to reproduce; keeping
   the floor a function of density preserves the physical coupling.
7. **Interpolation that is honest about what it can blend.** If ClothCraft ever
   blends two materials, the nonlinear curve must be blended explicitly (or the
   blend refused). The shipped behaviour of picking one side's curve while
   interpolating the scalars is the kind of quiet discontinuity we should not
   reproduce.
8. **A versioned schema from day one, with no reinterpretation of a slot.**
   CLO needed `_v2` keys and π/2/1000 rescaling precisely because a slot changed
   meaning. ClothCraft's material format should add a new field rather than
   change an existing one, and every loader should be property-tested against a
   round trip.

Explicitly out of scope and not to be copied:

* any CLO fabric preset, `.zfab`/`.zprj` payload or material table;
* the shipped default constants (this document deliberately omits them);
* CLO type names, key names as a public format, or the `_v2` compatibility
  scheme as our own.

Calibration data must come from the project's own measurements (or an explicitly
cited public source), and each recovered mechanic above should be validated by
ClothCraft's own tests — finite-difference consistency, objectivity, energy
balance, and the `m ≡ 1` linear limit — before any of it is claimed.

## 13. Reproduction

```bash
# Ghidra rejects universal Mach-O: thin each slice first.
FW=/Applications/CLO.app/Contents/Frameworks
lipo -thin arm64 "$FW/libCloSimulatePhysics.dylib"      -output clo3d_target/libCloSimulatePhysics.arm64.dylib
lipo -thin arm64 "$FW/libCloSimulateDataStructures.dylib" -output clo3d_target/libCloSimulateDataStructures.arm64.dylib
shasum -a 256 clo3d_target/libCloSimulatePhysics.arm64.dylib
# e6fd6f889b396b976810cc4267fd2b6823e99ec3767a1a781d5b91ce55d2ede1

# Structure without a disassembler:
nm clo3d_target/libCloSimulatePhysics.arm64.dylib | c++filt
strings -a clo3d_target/libCloSimulatePhysics.arm64.dylib

# The evaluator lives in a different library (kept outside the repo):
lipo -thin arm64 "$FW/libCloDeforMarvel.dylib" -output /tmp/lcdm.arm64.dylib
shasum -a 256 /tmp/lcdm.arm64.dylib
# c6c409c183c8f2d5a428a9efdd35ada1f9ee4a99d7018560ce8bd25b52df0f05

# REA / Ghidra route (one target at a time):
#   open_binary <slice>            (provider_id "ghidra")
#   binary_overview / list_procedures
#   libCloSimulatePhysics  : batch_decompile 0x3200 0x3354 0x34c4 0x3510 0x6de0 0x6e0c
#                            0x40fc 0x4144 0x4498 0x3844 0x69c4
#                            0x4558  /  0x55c4
#                            0x7134 0x74f4 0x9668 0x9d88  /  0x800c
#   libCloSimulateDataStructures : 0x3fb4 0x55c0 0x4470 0x5a1c
#   libCloDeforMarvel      : 0x3ed34 0x3edbc 0x1fa08  /  0x3ef38
```

## 14. Licence and provenance

The CLO artifacts analysed here are **not** distributed with this repository and
were not modified. All identifiers quoted above are symbol names recovered from
those artifacts and are recorded for interoperability research only. No file
under `src/`, `data/` or any CLO asset was touched by this pass.
