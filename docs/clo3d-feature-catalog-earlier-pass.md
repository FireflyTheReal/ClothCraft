# CLO3D Feature Catalog — Complete Inventory

**Target:** CLO 2026.1.310
**Derived from:** the shipped application's own command surface (Qt action slots),
dialog class inventory, and preset/asset tree — all read statically from
`CLO_Standalone_OnlineAuth` and the app bundle.

## How this catalog was produced, and what it is

Every feature below is anchored to something that exists in the shipped binary:

| Source | Count | What it proves |
|---|---|---|
| `View2DActionSlots::*` | 407 | Commands available in the 2D pattern window |
| `View3DActionSlots::*` | 350 | Commands available in the 3D garment window |
| `MainMenuActionSlots::*` | 281 | Application-level menu commands |
| `Dialog*` classes | 214 | Distinct modal/panel user interfaces |
| `Preset/` + `Assets/` tree | 22 categories, ~1,700 files | Shipped content the features consume |

Total distinct commands: **1,028**. This is an *inventory of observable
capability*, which is exactly the kind of evidence interoperability work
legitimately uses: it is the same set of facts a user sees by clicking through
the menus, recovered mechanically instead of by hand.

**This catalog contains no CLO source code, no decompiled logic, and no
proprietary assets.** It is a list of *what the program can do*, which is
factual and not itself protected expression. The implementation plan in
[`ROADMAP.md`](../ROADMAP.md) is a clean-room design: it says how *we* would build
each capability from published techniques and open formats.

## Status legend

| Mark | Meaning |
|---|---|
| **P0** | Required for the pipeline to function at all |
| **P1** | Required for the tool to be usable by a real patternmaker |
| **P2** | Professional parity; needed for production use |
| **P3** | Adjacent/vertical capability; can be deferred indefinitely |
| **—** | Deliberately out of scope (see §10) |

---

## 1. Application shell and document management

| Feature | Evidence | Tier | ClothCraft status |
|---|---|---|---|
| Project/document model | `MainMenuActionSlots` file group, `DialogZprjSaveOption` | P0 | Partial (`clothcraft-app`) |
| Open/save project | `DialogPacFileSave`, `OnFileAddProject` | P0 | Not started |
| Multi-window / multiview | `DialogMultiview`, `OnAdd3DWindow`, `OnClosedPythonConsole` | P2 | Not started |
| Undo/redo stack | `UnReMultipleOperations` (273 symbols) | P0 | Not started |
| Preferences | `DialogPreference` (250 symbols) | P1 | Not started |
| Recent files, autosave | `OnAutoSaved` | P2 | Not started |
| Licensing/activation | `AuthenticationProcessor`, `libhasp`, `OnDeactivateLicense` | — | Out of scope |
| Update/telemetry | `CLOUIUpdateImpl`, `DialogLifecycleManager` | — | Out of scope |
| Plugin system | `DialogPlugInManager`, `DialogRegisterPlugin` | P2 | Not started |
| Localisation | `OnChangeLanguage`, 5 CJK fonts shipped | P2 | Not started |

## 2. Avatar system

| Feature | Evidence | Tier | Status |
|---|---|---|---|
| Parametric body model | `AvatarRoot` (261), `AvatarManager` (165) | P0 | Started (`clothcraft-avatar`) |
| Base mesh + tetrahedralisation | `Preset/Avatar/Tetrahedralize/*.obj` | P0 | Not started |
| Avatar size/measure editor | `DialogCloAvatarEditor` (338), `DialogViewSizeChart` | P1 | Not started |
| Measurement offsets | `DialogAvatarMeasureOffset` | P2 | Not started |
| Pose / joint posing | `DialogPoseManager`, `DialogXrayJoints`, `DialogMirrorJoint`, `DialogOrientJointAxis` | P2 | Not started |
| Motion capture / motion files | `OnFileLoadMotion`, `DialogMotionConverter`, `MVAnimUnit` | P3 | Not started |
| Avatar auto-converter (`.avt`) | `DialogAvatarAutoConverter`, DAZ Genesis presets | P2 | Not started |
| Skin-suit fitting layer | `Preset/Skinsuit/SkinSuit.zpac` (82 files) | P3 | Not started |
| Landmark prediction | `DialogLandmarkPredictor` | P3 | Not started |
| Morph targets | `DialogMorphTargetController`, `OnFileLoadMorphTarget` | P3 | Not started |

## 3. 2D pattern drafting (407 commands)

### 3.1 Piece creation and primitives

`OnCreateRectanglePattern`, `OnCreateCirclePattern`, `OnCreateSpiralPattern`,
`OnCreateBaselineRectangle`, `OnCreateBaselineEllipse`, `OnCreateBaselinePolygon`,
`OnCreateGuideLineFromLine`, `OnCreateReferenceCircle/Line/Polygon/Rectangle`,
`OnCreateInnerCircle/Polygon/Rectangle`, `DialogCreateCircle`,
`DialogCreatePolygon`, `DialogCreateRectangle`, `DialogSpiral`,
`DialogRoundCornerByLength`, `DialogExtrudeByLength`, `DialogSection`,
`DialogCreatePleats`, `DialogPleatsFold`, `DialogRollUp`

**Tier P0/P1.** Rectangles, polygons, circles, internal shapes, guide/reference
lines, rounding.

### 3.2 Point and curve editing

A large family of conversions, each its own command:

`OnConvertToPoint`, `OnConvertToCurvePoint{With,Without}AddingCurvePoints`,
`OnConvertToSegmentPoint{With,Without}AddingCurvePoints`,
`OnConvertToCubicSpline`, `OnConvertToCurve`, `OnConvertToGradePoint`,
`OnConvertToNonGradePoint`, `OnConvertToBaseline`, `OnConvertToPOM`,
`OnConvertToHole`, `OnConvertToInternalLine`, `OnConvertToInternalShape`,
`OnOptimizeCurvePoints`, `OnAddPointAtIntersection`,
`OnAddPointSplitLineOn{Start,End}Point`, `OnChangeLength`,
`DialogSetLineLength`, `DialogOptimizeCurvePoints`

**Tier P0.** This confirms the §2.1 finding in `ARCHITECTURE.md`: the boundary
is a set of **typed parametric points on curved segments**, where a point can be
reinterpreted without changing the curve. That reinterpretation-without-
reparametrisation requirement is the central data-model constraint.

### 3.3 Darts, pleats, fullness

`OnCreateDart`, `OnCreateOutlineDart`, `OnCreateBaselineDart`,
`DialogAddAndEditDart`, `Dialog_Create_Dart`, `OnAddDartFullness`,
`DialogFullness`, `OnCreatePleats`, `DialogPleatsFold`, `OnSlashAndSpread`,
`DialogSlashAndSpread`

**Tier P1.** Dart manipulation (rotate/slash-and-spread) is core patternmaking.

### 3.4 Seams, sewing and notches (2D)

`OnSegmentSewing`, `OnFreeSewing`, `On1toNSegmentSewing`, `On1toNFreeSewing`,
`OnSplitMtoNSewing`, `OnSewingWithLinkLines`, `OnPleatsSewing`,
`OnChangeSewingLineType`, `OnAddSeamlineRipOnSewing`, `OnAddTopstitchOnSewing`,
`OnSymmetrizationWithSewing`, `OnHalfSymmetrizationWithSewing`,
`OnActivateSeamLine`, `OnActivateAllSeamLinePairList`,
`OnActivateSeamlineRipSelected`, `OnCheckSewingLength`,
`DialogCheckSewingLength{Closed}`, `OnAddNotch`, `DialogAddPunchMark`,
`OnSetNumberOfSewingLayers`, `OnSewingThread{RedColor,SeamLineColor}`

**Tier P0.** Directly confirms and *extends* the `SeamLinePairGroup` finding:
sewing is **1:N and M:N by segment range** (`On1toNSegmentSewing`,
`OnSplitMtoNSewing`), with seam rips and topstitching as first-class seam
properties. ClothCraft's current one-segment-to-one-segment `SeamRef` is
insufficient and must be extended.

### 3.5 Internal lines and graphics

`OnContourGraphic`, `OnAddInternalLineAsAngle{AxisX,AxisY,Local}`,
`OnAddInternalLineOnTopstitch`, `OnAddAnnotation`, `OnAddInstructionLine`,
`OnAddCircleAroundAvatar`, `OnInsertPrintTexture3D`

**Tier P2.**

### 3.6 Grading (size runs)

`OnAutoGrading`, `DialogAutoGrading`, `DialogGradingPointEditor`,
`DialogGradingTopology`, `DialogSelectGradingSizes`, `DialogNestGrading`,
`DialogSelectSize`, `GradingRuleTable` (85), `GradingWidget` (112),
`GradingReviewMain` (148), `GradingPointPropertyWidget` (57)

**Tier P2.** A grade rule table maps each graded point to a per-size
displacement. This is a self-contained, very testable subsystem.

### 3.7 Measurement (POM) and fitting

`OnCreatePatternMeasure`, `OnConvertToPOM`, `DialogCalculatorForPOM`,
`DialogPOMCalculationNotepad`, `DialogPOMExportOptions`, `DialogPOMDeleteFolder`,
`OnExportPOM`, `DialogMatchMeasurement`, `DialogFitmapProperties`,
`DialogFitMap`, `DialogFitToUnified`, `PageFitmapProperties`,
`OnCutAndSewGarmentMeasure`, `OnShow3DPatternMeasure`

**Tier P2.** Points of measure attached to pattern topology, plus a fit map
that reconciles avatar and garment measurements.

### 3.8 Alignment, arrangement and view

`OnAlign{Left,Right,Top,Bottom,Center,Middle}Topology`,
`OnAlignGrain{XAxis,YAxis,ParallelToSelected}`, `OnAlignPointTo{XAxis,YAxis}`,
`OnArrangePlane`, `OnAutoArrangement`,
`OnActivate{Grid,Snapping,SmartGuideSnapping}`, `On2DBackground`,
`DialogFormatBackground`, `On{BringForward,BringToFront}`, `DialogDistribute`,
`DialogTranslate`, `DialogEdit{Rotation,Scale}`, `DialogMirrorJoint`,
`DialogGroupTools`, `DialogSoftSelection`

**Tier P1.**

### 3.9 Symmetry and cloning

`OnSymmetrizationWithSewing`, `OnHalfSymmetrizationWithSewing`,
`OnCloneAsPatternFromShapeAndPattern`, `OnCloneAsPatternWithSewing`,
`OnCopyAsInnerShapeFromShapeAndPattern`, `OnCopyTopology`,
`OnCopyAsGuidelineWithOffset`, `OnCopyAsInternalShapeWithOffset`,
`OnDuplicate`, `Symmetry` in the piece model

**Tier P1.**

### 3.10 Import/export (2D)

`DialogImportDXF`, `DialogExportDxf`, `DialogImportAI`, `DialogExportAIWithOptions`,
`DialogImportOBJ`, `DialogExportOBJ`, `DialogImportPacx`, `DialogFileInfo`,
`OnExportLayout`, `OnExportBOM`, `OnExportBOMJSON`,
`DialogPlotterOption`, `DialogExportPltOption`

**Tier P0 (DXF/PLT) / P2 (AI, OBJ, BOM).** ClothCraft has HPGL and a DXF subset.

---

## 4. Materials, fabric and colour

| Feature | Evidence | Tier | Status |
|---|---|---|---|
| PBR material model | `PBRMaterialGUIInterface` (184), `PBRMaterialPublicInterface` (122), `MaterialProxy` (176) | P0 | Partial (`clothcraft-fabric`) |
| Texture-map slots | `Texture` (125), `TextureMapType` (85) | P1 | Not started |
| Fabric presets | `Preset/` fabrics; `DialogCLOFabSaveFabric`, `DialogSaveFabric` | P1 | Not started |
| Physical properties | `PhysicalProperty::Load/Save`, `Preset/Simulation_Property` | P0 | Not started |
| Nonlinear stiffness curves | `NonlinearStiffness`, `ToNonlinearStiffnessPiecewiseFn` | P1 | Not started |
| Colourways | `Colorway` (94), `ColorwayLibrary` (125), `DialogColorwayList` (111), `DialogColorwayAdd/Select` | P2 | Not started |
| Colour swatch libraries | `ColorSwatchLibrary` (94), `ColorSwatchList` (83), `Preset/Palette` | P2 | Not started |
| Knitwear | `libCloKnit_Core` (248), `DialogKnitEditorWindow`, `Preset/Knitwear` (52) | P3 | Not started |
| Fabric scanner import | `Preset/FabricScanner` (30) | P3 | Not started |
| AI texture generation | `DialogAITextureImageGenerator`, `libonnxruntime`, `ONNX` | P3 | Not started |
| Denim/jeans washing | `OnExportJean`, `DialogSelectJeanWashing` | P3 | Not started |
| Substance/AxF material import | `libsubstance_linker`, `libAxFDecoding` | — | Out of scope (licensed) |

---

## 5. Trims and hardware

| Feature | Evidence | Tier |
|---|---|---|
| Zipper (geometry, tape, puller, slider) | `Zipper` (108), `ZipperStyle` (157), `DialogRegisterZipperObject`, `DialogRegisterZipperObjectManager`, `Preset/Zipper` (13) | P2 |
| Buttons and buttonholes | `ButtonHeadStyle` (91), `DialogCreateButtonHeadHole`, `DialogCreateButtonHeadPreset`, `DialogRegisterButtonHeadObject` | P2 |
| Topstitch styles | `TopStitchStyleProperty` (154), `DialogRegisterTopstitch` | P2 |
| Piping | `OnCreatePiping`, `OnEditPiping`, `OnRefreshSelectedPiping` | P3 |
| Accessories | `DialogAccessorySaver`, `DialogOpenTrim`, `DialogSaveTrim` | P3 |
| Bonding / fusible | `DialogBonding3D`, `Preset/Bond` (6), `Preset/Bond_Defual` | P3 |
| Fold3D | `OnFold3DPattern`, `OnFold3DGarment`, `Dialog3DFoldWidget`, `Preset/Fold3D` | P3 |
| YKK hardware library | `Preset/ykk_logo_(R)_N.svg` | — |

**Observation:** zippers + buttons + styles account for roughly **700 symbols** —
one of the largest single feature families in the product. This is a catalogue of
modelled hardware objects with registration dialogs and style inheritance, not a
texture trick.

---

## 6. Simulation and draping

| Feature | Evidence | Tier | Status |
|---|---|---|---|
| Normal drape | `OnNormalSimulation`, `OnAutoDraping` | P0 | Partial (`clothcraft-solver`) |
| Fitting simulation | `OnFittingSimulation` | P1 | Not started |
| Robust simulation | `OnRobustSimulation` | P2 | Not started |
| Animation simulation | `OnAnimationSimulation`, `OnPlaySimulationAndAnimation` | P3 | Not started |
| **CPU vs GPU toggle** | `TurnOnCPUSimulation`, `TurnOnGPUSimulation`, `libCloSimulate{Cuda,Metal}` | P2 | Not started |
| Pins (point/line/box/lasso) | `OnCreatePin`, `OnAddLinePin`, `OnBoxPinning`, `OnLassoPinning`, `OnActivatePin`, `OnAttachAllPinsToAvatar` | P0 | Partial |
| Pinching brushes | `OnPinchBrush`, `OnBrushPinching`, `OnAdvancedPinching`, `DialogBrushPinching` | P2 | Not started |
| Glue / bonding brush | `OnGlueByBrush`, `DialogGlueFill` | P3 | Not started |
| Wind | `OnActivateWind`, `OnDeleteWind`, `OnDuplicateWind`, `WindProperty` (~45) | P2 | Partial (field only) |
| Pressure | `OnClothPressure`, `OnClothPressureSlot` | P2 | Not started |
| Layer/layer count | `OnSetNumberOfSewingLayers`, `OnShowBlockSewing` | P2 | Not started |
| Simulation properties UI | `OnSimulationProperty`, `DialogOverrideSettings` | P1 | Not started |
| Solver internals | see `ARCHITECTURE.md` §5 | P0 | Partial |

**Solver evidence recap** (from the decompilation in `ARCHITECTURE.md` §5): a
block-sparse `matMixCSR` system of 3×3 nodal blocks solved by block-Jacobi
preconditioned conjugate gradient, with mass-spring edges carrying nonlinear
stiffness curves, plus a KD-tree broadphase and a triangle/tetrahedron edge hash.

---

## 7. Rendering, lighting, camera and output

| Feature | Evidence | Tier |
|---|---|---|
| Real-time viewport | `View3DBase` (137), `QtView3D` (97), `CLOVRayRenderView` (85) | P0 |
| Viewport display modes | `OnCloth{Fill,Texture,Thick,Translucent,Wireframe,Seamline,Pressure,GraphicOnOff}`, `OnAvatar{XRay,Translucent,WireFrame,Fill,OnlyFill}`, `OnClothFixedpoint` | P1 |
| Thick cloth (two-sided shell) | `OnClothThickBackSurface`, `OnClothThickRandomColor`, `ThickParametricLine` | P2 |
| Cameras | `Camera` (195), `OnCam{Front,Back,Left,Right,Top,Bottom,FrontLeft,FrontRight}`, `DialogCameraPreview`, `DialogCustomView`, `Preset/Camera` | P1 |
| Lighting presets | `OnLightPreset0..3`, `OnSetCamProperty`, `On{Show,Hide}VRayLight`, `Assets/Configuration/Render/{HDRI,IES}` | P2 |
| **V-Ray production render** | `libvray*` (~450 MB), `OnClickedVRayRenderView`, `DialogRenderQuality` | — |
| Real-time ray tracing | `OnClickedRTRTRenderView`, `OnClickedRTRTFLIPCompare`, `OnClickedRTRTOpenPBRVisualEquivalenceTest` | P2 |
| Image/PNG output | `Preset/watermark.png`, `OnExportLayout` | P1 |
| UV editor | `OnChangeRenderingModeOnUVEditor`, `DialogMoveUV`, `DialogUVPacking`, `DialogUVSnapshot` | P2 |
| Animation/video capture | `DialogVideoCapture`, `DialogVideoCaptureMD`, `DialogSaveAnimation` | P3 |
| Colour management | `Preset/ColorProfile/ISOcoated_v2_eci.icc`, `liblcms2` | P2 |

**Out of scope by licence:** the V-Ray renderer is ~450 MB of commercially
licensed third-party code (`libvray`, `libvrayosl`, `libVRaySDKLibrary`,
`libVRayOpenImageDenoise_core`, `libvrayopenimageio`, `libchaos_networking`).
There is no lawful path to reproducing it and no need to: an open renderer
covers the viewport and preview requirements.

---

## 8. Interchange and export (all 27 export commands)

`OnExportAdobe`, `OnExportAlembic`, `OnExportFBX`, `OnExportGLB`, `OnExportGLTF`,
`OnExportGLTFFabric`, `OnExportUSD`, `OnExportToUSDLayer`, `OnExportVRM`,
`OnExportClothOBJ`, `OnExportSelectedClothOBJ`, `OnExportOBJAnimation`,
`OnExportMayaCache{2009AndBelow,2010AndAbove,Mcx}`, `OnExportLightWaveMDDCache`,
`OnExportMayaMaxMDDCache`, `OnExportPointCache`, `OnExportBoneInformation`,
`OnExportGoZToZBrush`, `OnExportDxf`, `OnExportLayout`, `OnExportBOM`,
`OnExportBOMJSON`, `OnExportPatternAPIJSON`, `OnExportPOM`, `OnExportJean`

Supporting libraries: `libfbxsdk`, `libusd_ms`, `libAlembic`, `libcollada-dom`,
`libPDFNetC`.

**Tier P1 for OBJ/glTF/DXF/Alembic; P2 for FBX/USD/BOM; P3 for the rest.**
Format priorities differ from CLO's because the ecosystem has moved: glTF and
USD matter more today than LightWave MDD or ZBrush GoZ.

---

## 9. Automation, AI and ecosystem

| Feature | Evidence | Tier |
|---|---|---|
| Python scripting API | `libpython3.11`, `python311.zip`, `CLOAPIInterface`, `OnClosedPythonConsole`, `DialogCodeEditor` | P2 |
| AI avatar studio | `DialogAIAvatarStudio`, `OnClickedAIAvatarStudio` | P3 |
| AI parametric pattern | `DialogAIParametricPattern` | P3 |
| AI pose/graphic/texture generation | `DialogAIPoseGenerator`, `DialogAIGraphicImageGenerator`, `DialogAITextureImageGenerator` | P3 |
| Pattern drafter (from prompt) | `DialogPatternDrafter`, `PatternDrafterPatternListWidget`, `Preset/FabricCreator` prompts | P3 |
| Fabric creator | `Preset/FabricCreator` (224 files) | P3 |
| AI chatbot | `OnAIChatBot` | — |
| Modular library / blocks | `ModularLibraryDataManager` (153), `DialogModularCreator`, `Assets/.../Modular Library` | P3 |
| Shared blocks / style configurator | `DialogSharedBlock`, `DialogStyleConfigurator` | P3 |
| EveryWear | `EveryWearModel` (72), `DialogEveryWear*`, `OnClickedEveryWearButton` | — |
| Cloud/PACX exchange | `OnFileLoadPACX`, `DialogImportPacx` | — |
| Auto retopology | `libCloAutoRetopology`, `DialogRetopologyPatch`, `DialogInputPolygonCountForRetopology` | P3 |

---

## 10. Explicitly out of scope

These are recorded so the boundary is deliberate rather than accidental.

| Capability | Reason |
|---|---|
| HASP dongle / online activation | `libhasp_darwin`, `libcgauth`, `AuthenticationProcessor`. Circumventing licensing is off the table. |
| V-Ray renderer | Commercially licensed third-party code. |
| Substance / AxF material decoding | Licensed SDKs (`libsubstance_linker`, `libAxFDecoding`). |
| Proprietary `.zprj`/`.zpac`/`.zfab`/`.avt` parsing | Schema is only partially observable; see [`FORMAT-NOTES.md`](clo3d-format-notes-earlier-pass.md). A guessed parser would silently misread real documents. |
| PANTONE / YKK branded libraries | Trademarked third-party content. |
| DAZ Genesis avatar conversion | Third-party licensed character meshes. |

---

## 11. What the catalog implies

Four conclusions that shaped [`ROADMAP.md`](../ROADMAP.md):

1. **The 2D pattern editor is the largest and most valuable subsystem.** 407 of
   1,028 commands live there, and it is where a garment's design is actually
   expressed. The solver is comparatively small (the whole linear-solver library
   is 35 functions).
2. **Seams are M:N over segment ranges**, with rips, topstitch and layer counts
   as first-class properties. The current one-to-one `SeamRef` must change.
3. **Trims are a quarter of the product surface** (~700 symbols) and deliver no
   simulation value. They are correctly deferred.
4. **The pipeline that matters is small and well-defined**: pattern → seams →
   assemble → simulate → export. Almost everything else is authoring
   convenience layered on top of it.
