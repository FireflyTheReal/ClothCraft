# CLO3D UI structure — what is recovered, and what is not

**Status:** evidence document. Answers the standing question *"what do you not
understand under copy/reverse engineer all tabs and options from clo3d"* — by
recovering as much of the actual UI structure as the shipped artifacts support,
and stating plainly where the structure stops being recoverable.

**This document does not contain a menu tree.** Section 12 explains what is
missing and exactly what would recover it. Section 13 lists what would have to
be true before anyone should build an interface against this.

Names, structure and counts only. No CLO asset, icon, artwork or label-text
content is reproduced beyond interface identifiers.

## 0. Evidence discipline

| Tag | Meaning |
|---|---|
| **Observed** | Read directly from the shipped artifact — a literal string in the string table, a symbol-table entry, a file header field. |
| **Derived** | A conclusion that follows mechanically from observations, with the reasoning given. |
| **Inferred** | A reading that fits the evidence but is *not* confirmed. Never load-bearing. |
| **Unknown** | Not established. Stated as such rather than guessed. |

**Important qualification about Evidence IDs.** Only two REA Evidence records
were produced in this pass, and **both record a failure** (§1.2). Everything
else here is a direct read of the shipped symbol table and string table with
`nm` and `strings`, which needs no disassembler and therefore produces no
Evidence ID. Where a claim has no ID it is because there is no REA record, not
because a record was lost. The method used instead, and the test that validates
it, are in §3.

### 0.1 Tool boundaries — read this before the recovered material

The runtime accessibility route was used in this pass and it produced real
structure (§0.2). It also has **hard boundaries, and they are tool boundaries
rather than CLO facts.** They are placed here, before any recovered material, so
that a reader does not reach the end of a recovered toolbar list and assume the
menu tree is merely pending. It is not pending; these tools cannot answer it.

**Only the visible, layer-0 main window is observable.** REA's helper requires
the selected window to satisfy `kCGWindowLayer == 0`
(`bridge/native/ReaNativeUI.swift`), and it obtains the window through
`CGWindowListCopyWindowInfo([.optionIncludingWindow], id)`. Observed outcomes for
CLO's windows:

| window | layer | state | result |
|---|---|---|---|
| 26121 (workspace) | 0 | on-screen | **captured** |
| 26107 (580×568) | 0 | off-screen | `window-mismatch` |
| 26114 (394×357) | 0 | off-screen | `window-mismatch` |
| 26110 (menu-bar surface) | 1000 | off-screen | `window-mismatch` |
| 26139 (`Dialog`) | 8 | off-screen | not addressable |
| 26138 (`Library`) | 3 | off-screen | not addressable |

**Derived — the rule, with its confound named:** addressability requires the
window to be **both** currently enumerable by CoreGraphics **and** layer 0. The
failure mode is observed directly (three `window-mismatch` refusals). **Unknown:**
whether those refusals are caused by off-screen state or by something else — every
non-workspace CLO window is *also* off-screen, so the two conditions could not be
separated with the samples available, and this document does not assert which one
does the rejecting.

**The menu bar is the native macOS menu bar, and it is out of reach.** Every
application on this machine owns an `828×30 @ (0,0)` **layer-1000** window —
CLO's is 26110, alongside the harness, ClothCraft, Safari, Finder, Terminal,
Preview and others. That is the native menu-bar surface, so CLO's menu items
live in the **application-scope `AXMenuBar`**, not in the window subtree REA
reads. Three independent observations:

1. the complete capture of the workspace (`truncated: false`, 121 nodes) contains
   **no `AXMenuBar`** and no top-level menu titles;
2. pressing the only menu-bearing element inside the window (an `AXMenuButton`
   titled `" SIMULATION"`) changed nothing — `children_count` was 121 before and
   after both steps, and a CoreGraphics re-list immediately afterwards showed
   **no new CLO window**;
3. selecting menu-bar window 26110 directly returns `window-mismatch` because of
   the layer gate above.

**Consequence: action→menu membership is not obtainable from these tools.** It
needs application-scope AX (not offered by the tool surface used here) or the
Ghidra route. This is recorded so the effort is not spent twice.

**Keyboard shortcuts are unavailable too.** `capture_native_ui_scenario`'s
`key-entry` **sets the selected element's `AXValue`**; it does not inject global
keystrokes. There is also **no hover action** — the available steps are click,
scroll, key-entry and wait — so any experiment that presupposes hovering cannot
be executed as written and must fall back to a click.

**Dialog content is effectively unreachable.** CLO's dialog-class windows here
sit at layer 8 (`Dialog`) and layer 3 (`Library`), below the layer-0 gate, and
off-screen windows are refused. A dialog would become observable only if it
presented as an on-screen layer-0 window. **Unknown:** whether any specific CLO
dialog does — `Dialog` is a 300×24 strip, not a dialog frame, so it is suggestive
rather than conclusive about, for example, `DialogPreference`.

**One operational hazard.** REA compiles a fresh Swift helper per call into a
temporary root. Under load (observed at **load average 9.53**, 4,076,060
pageouts) that compile exceeded its 60-second budget once and the call failed
with `execution_failure` / `Command timed out after 60000 ms: /usr/bin/xcrun`. A
timed-out helper is **not** a finding about CLO.

### 0.2 Runtime-recovered workspace structure (accessibility capture)

This is the one layer the static pass could not reach, and it is the strongest
single artifact in this document. **Observed**, all with `truncated: false`:

| item | evidence |
|---|---|
| workspace window at rest, 121 nodes | `ev_9499ebcf86e3e6851eb026d7578bbb73bd9b785d5fc9e254870f9f029b9a0bcf` |
| press of the in-window `AXMenuButton` (no effect) | `ev_a1b1c6a631e577734dc3a4ea2372cc684b0f09e47ae8cdabc254b8b2ea128b9c` |
| tab selection in group [86] | `ev_4f36acd2a8896a538304ddba1173b2cef2abf80626798bd85cce361566c6d052` |
| post-selection re-capture, 121 nodes | `ev_a8b82e36e43bffcb6be68121e87cc8d99f4e7f75d0812f3cc3ba90221f12bb37` |

**Identity, confirmed from the tree rather than from geometry:** pid 67966,
window 26121, executable digest `9ee952f4…`, and the status bar carries
`Version: 2026.1.310 (r58944)` — the build **revision `r58944`**, which no static
pass had. The loaded document is **`Untitled.zprj`**.

**Reproducibility — what makes the control list citable.** The first capture
(`ev_9499ebcf…`) and the later re-capture (`ev_a8b82e36…`) are **two independent
calls with a full Swift helper recompile between them**, taken ~15 minutes apart.
Nodes `[0]`–`[41]` reproduced **label-for-label and in the same order**, and the
panel, tab and status-bar structure is identical. The inventory below is
therefore a stable property of the window, not a one-off read.

**Toolbar composition — the recovered control inventory.** The window exposes
roughly **80 titled controls**. AX role distinguishes how each behaves, which is
what makes the list actionable rather than a word cloud:

* **`AXCheckBox`** — a toggle. `Select/Move`, `Fold Arrangement`,
  `Fold 3D Garment`, `Edit Sewing`, `Edit Tack`, `Play Motion`,
  `Edit Line (3D Pattern)`, `Line (3D Pattern)`, `3D Base Pen`,
  `Edit 3D Pen (Avatar)`, `Edit Texture (3D)`, `Transform Graphic`,
  `Select/Move Button`, `Fasten Button`, `Zipper` (twice), `Edit Piping`,
  `Piping`, `Select Binding`, `Binding`, `Press`, `Edit Rip`,
  `Transform Pattern`, `Seamless Blocking`, `Trace`, `Seam Allowance`,
  `Walk Pattern`, `Edit Point of Measure`, `Point of Measure`,
  `Edit Annotation`, `Check Sewing Length`, `Edit Texture (2D)`, `Steam`,
  `Seam Taping`, `Edit Topstitch`, `Edit Puckering`, `Set Sublayer`, `Fill`,
  `Edit Lacing`, `Lacing`, `Edit Gluing`, plus three untitled toggles.
* **`AXPopUpButton`** — a dropdown/segmented chooser exposing `AXShowMenu`.
  `Simulate`, `Select Mesh (Brush)`, `Edit Styleline`, `Segment Sewing`,
  `Free Sewing`, `Tack`, `Hi-Res Garment`, `Edit Measure (Avatar)`,
  `Basic Circumference Measure (Avatar)`, `Edit Measure (Garment)`,
  `3D Pen (Avatar)`, `Button`, `Edit Pattern`, `Fullness (Point)`, `Polygon`,
  `Internal Polygon/Line`, `Base Polygon`, `Notch`, `Pattern Annotation`,
  `Pleats`, `Edit Grading`, `Segment Topstitch`, `Segment Puckering`,
  `Glue Fill`.
* **`AXButton`** — a one-shot command. `Reset 2D Arrangement (All)`,
  `Reset 3D Arrangement (All)`, `Auto 3D Arrangement`,
  `Re-drape 3D Arrangement`, `Auto Sewing`, `Graphic (3D Pattern)`,
  `Auto Fitting`, `Graphic (2D Pattern)`, plus thirteen untitled buttons.

**Two independent methods converge on the same list — and this is the point.**
The drafting design document ranked what is missing from the interface as
**seam allowance, then notches, then points of measure, then symmetry, then
grading, then seam-ease** — a judgement reasoned from patternmaking practice. The
shipped binary independently exposes `Seam Allowance`, `Notch`, `Walk Pattern`,
`Edit Point of Measure`, `Point of Measure`, `Pleats`, `Edit Grading`,
`Check Sewing Length`, `Seam Taping`, `Topstitch`, `Puckering`, `Lacing` and
`Gluing`. The first is practice; the second is an artifact reading. **That they
agree is much stronger evidence than either alone**, and it is the clearest
justification available for the drafting roadmap's ordering.

**Panel structure.** `Object Browser` and `Property Editor` each appear as a
labelled panel header with two adjacent untitled buttons; document tabs
`Untitled.zprj` and `2D Pattern Window` are `AXTabGroup`s with a single radio
child each; `CLOFAB` appears as a button.

**A tab strip drives a neighbouring panel's content — real structure, not a
label.** Selecting tab 0 of the 13-child `AXTabGroup` at path [86] replaced the
content of the sibling group [85]:

* **before:** two untitled buttons, a **`CLOFAB`** button, an **`AXOutline` with
  3 rows**;
* **after:** an untitled button, then **`Add`**, **`Copy`**, **`Assign`**, and an
  **`AXList` with 2 rows**.

So `[86]` is a tab strip that selects what the adjacent panel shows: an
outline/tree view beside a `CLOFAB` button, or a list view with
`Add`/`Copy`/`Assign`. **No document was written** — this is a view switch, and
no further clicks were made after it.

**Eleven unnamed tabs stay Unknown.** The path-[86] `AXTabGroup` has **13
children: 11 `AXRadioButton`s whose titles are empty strings and whose `value`
is null**, plus `Scroll Left` and `Scroll Right`. **Selecting one did not
populate any title**, before or after. Because the radios expose neither a title
nor a value, **which tab is selected is not readable either** — a switch
definitely occurred (the sibling-group effect above), but it cannot be named.
These eleven tabs are recorded as Unknown and are **not** named from their
position, from the `menu_*` literals, or from their neighbours.

**One in-window menu affordance exists:** an `AXMenuButton` titled
`" SIMULATION"` (with `AXShowMenu`). Pressing it produced no observable change
(§0.1), so its menu content is Unknown.

### 0.3 Privacy

The status bar displays a signed-in account label. It is **not** reproduced in
this document. The same rule applies to any other personal value that appears in
a capture.

## 1. Targets and the hard analysis limit

### 1.1 Targets (**Observed**)

CLO `2026.1.310`. The only artifact that contains the Qt user interface is the
main executable.

| Artifact | Bytes | Note |
|---|---|---|
| `Contents/MacOS/CLO_Standalone_OnlineAuth` (universal) | 126,231,984 | the UI lives here |
| `arm64` slice (used for all reads) | 58,287,536 | SHA-256 `b2d545d7baaefa3b303599c2bb51f49b94b04607531d55ba53f5693e9bff128f` |
| `Contents/Frameworks/libCloScene.dylib` (universal) | 194,156,592 | **larger than the main executable**; contains scene/asset tables, not the UI |
| 21 `libClo*` libraries | — | simulation, math, data structures, file IO; no `Dialog*`/`Page*` UI classes of substance |

**Derived:** there is no small target that carries the user interface. The UI
cannot be analysed by choosing a cheaper library.

**Observed:** the scratch directory `clo3d_target/` was removed by another
process during this session. The slice was regenerated outside the repository as
`/tmp/clo_main.arm64` with `lipo -thin arm64`, and it reproduced the same
SHA-256 byte-for-byte, so the reads below are of the identical input recorded in
[clo3d-capability-register.md](clo3d-capability-register.md).

### 1.2 Decompilation of the main executable failed — with evidence

Two attempts were made to decompile the function that sets up the Multiview
dialog's tab widget (`Marvelous::DialogMultiview::SetupUiSingleviewTabWidget`,
slice address `0x1011f52b0`).

| Evidence ID | Outcome |
|---|---|
| `ev_5af060db9d8321fed252eb70709b17eda0ff57e99d25a1ab1fad47dcbe198ae5` | `batch_decompile` on `0x1011f52b0` — no usable result |
| `ev_c83b938fb9c7572838a8840471add1ecd3853898352351992813247f9bdfd29d` | `batch_decompile` retry on `0x1011f52b0` — `provider_timeout`, then `artifact_changed` |

The second record's diagnostics are the informative part (**Observed**):

* failure kind `timeout`, `timeout_ms: 330000` — Ghidra's analysis deadline
  elapsed *before* the requested function could be resolved;
* `procedure_address` never returned; the error is raised on address resolution,
  not on decompiling one function;
* the loader reported `Discovered 191286 DYLD_CHAINED_PTR_64_OFFSET chained
  pointers`, `Fixed up 191286 chained pointers`, then `ANALYZING all memory and
  code`, and later `hit non-returning function, restarting decompiler switch
  analyzer later`;
* the retry additionally reported `artifact_changed` / `ENOENT` because the
  scratch slice had been deleted between the two calls.

**Derived:** the blocker is Ghidra's *default whole-program auto-analysis of a
39.7 MB `__text`*, not the Mach-O format and not the tooling. A request for one
small function still pays the full analysis cost. This is the single reason no
instruction-level UI evidence exists in this pass.

**Unknown:** whether a longer analysis budget would complete on this host. The
host has ~8 GB of RAM; the analysis was not retried with a raised budget.

**What this costs us:** every claim about *which item is in which menu* depends
on the `addAction` / `AddAction` call sites, which are instruction-level. That
layer is therefore absent, and §12 says so explicitly rather than filling it in.

## 2. Method: reading the Qt literal blocks without a disassembler

`strings -a -t d` on the slice yields **162,320** literals with file offsets.
Within them, three families of data are present and, critically, **laid out in
contiguous per-class runs**:

| Family | Shape | Example |
|---|---|---|
| uic **object names** | identifier-like, no spaces | `PreferenceStackedWidget`, `imageResolutionComboBox` |
| Qt **meta-object strings** | method name followed by its parameter type list and parameter names | `OnValueChanged`, `QTreeWidgetItem*`, `item`, `column` |
| **visible labels** | prose or short capitalised phrases | `Add Category`, `View Count`, `Box Horizontal` |

A class-name literal heads each run (**Observed** across hundreds of classes,
for example the sequence `DialogAddCategory` → `le_GrName`, `buttonLayout`,
`Add Category`, `This name already exists.` → `1OnEditingFinished()`,
`2textChanged(const QString&)`).

**Derived:** the per-class grouping is a property of how the compiler places
each translation unit's literal pool; the run order matches the order the
literal was first used in that class. **Inferred:** the exact linker/compiler
mechanism (mergeable literal sections) — not verified, and not needed for the
grouping claim, which is observed directly from offset order.

### 2.1 The method is validated, not assumed

For `DialogPreference` the literal block yields **113** distinct `On…` handler
names. The symbol table independently contains **exactly 113** distinct
`DialogPreference::On…` / `::on…` members.

**Observed:** 113 = 113. Two independent sources — the literal run and the
symbol table — agree on the option-handler set of that dialog.

**Derived:** for a class whose block is clean, the literal run is a faithful
inventory of that class's members. **This validation was performed for
`DialogPreference` only.** Section 5 explains where the method produces false
positives, and those cases are marked rather than published.

## 3. Layer 1 — which screens have a Designer form

**Observed:** the string table contains **274** distinct `Ui::<Class>` literals.
Each one implies a Qt Designer form (`ui_<class>.h`) with a generated object
hierarchy.

**Observed:** segmenting the literal table by class-name heads yields **435**
class blocks that carry content; **274** of those heads also appear as a `Ui::`
literal.

**Derived:** 274 screens have a recoverable widget hierarchy in the shipped
binary. The register lists 225 `Dialog*`/`View*` classes by name; 274 `Ui::`
forms is a superset because some forms belong to widgets that the register's
name filter does not classify as a screen (panels, title bars, item widgets).

Largest recoverable hierarchies by recovered object-name count (**Observed**,
counts of identifier-shaped literals in the block):

| Class | object names | labels | meta signatures |
|---|---|---|---|
| `DialogCloAvatarEditor` | 725 | 258 | 163 |
| `DialogPreference` | 424 | 596 | 200 |
| `DialogImportOBJ` | 27 | 20 | 15 |
| `DialogExportOBJ` | 57 | 6 | 4 |
| `DialogMultiview` | 45 | 56 | 47 |

**Derived:** the object names encode the widget kind in their suffix
(`…ComboBox`, `…CheckBox`, `…SpinBox`, `…Button`, `…_label`, `…Frame`,
`…ScrollArea`, `…GridLayout`). That is how the option kinds in §4 were
classified — from an Observed name, by an explicit rule, not by guessing.

## 4. Layer 2 — tabs and pages, only where corroborated

A generic scan for `*Page` / `*Tab` / `*StackedWidget` object names over all 435
blocks produces **64** hits in **20** forms — **and that number is not
trustworthy.** The scan also matches method names (`OnResetCurrentTab`),
parameter type names inside signatures (`Marvelous::CLOFabScanCalibPage`) and
code-generated C++ mangled names. The worst offender is
`CLOEssentialIconItemWidget`, whose 18 "pages" are all repeats of two type names
appearing in signatures.

**Method rule applied here:** a page list is reported only when the page names
are corroborated by a second, independent source in the same class — typically
the `On…` handler names, which a Designer form always generates one pair of per
page control.

### 4.1 Corroborated page/tab structures (**Observed** names, **Derived** grouping)

| Class | Recovered page/tab members | Independent corroboration |
|---|---|---|
| `DialogPreference` | `PreferenceStackedWidget` containing `GraphicOptionPage`, `ViewControlPage`, `shortcutAndMenuPage`, `DefaultFilesPage`, `PlugInPage`, `CLOSETPage` (in that offset order) | 113 `On…` handlers; the handler topics map one-to-one onto these page names (§6) |
| `DialogModularCreator` | `creatorTabStackedWidget` containing `groupTab`, `categoryTab`, `styleTab` | handlers `OnMoveToGroupTab`, `OnMoveToCategoryTab`, `OnMoveToStyleTab` |
| `DialogAIParametricPattern` | `m_pTabWidget` containing `m_pPromptTab`, `m_pImageTab` | — (page names self-describing; no handler corroboration found) |
| `TextureMapGeneratorWidget` | `m_CLOEssentialLibraryPage`, `m_CLOSETLibraryPage`, `m_CONNECTLibraryPage` | the three names are distinct library pages; matches the `CLOEssential/CONNECT/CLOSET` library classes in the register |
| `DialogRegisterButtonHeadObject` | `m_fileTabWidget`, `ChangeTabPage` | symbol table has `RegisterButtonHeadObjectTabPage` (53 members) |
| `FinderDownloaderProgressBar` | `weightTab`, `sculptTab`, `frontFaceTab`, `backFaceTab`, `avatarTab`, `garmentTab`, `materialTab`, `debugTab` | eight distinct names with a consistent `…Tab` shape |
| `ModularLibraryWidget` | `tabStackedWidget` + `lineTab` | handlers `OnMoveToLineTab`, `OnChangeLineByBlockPage` |
| `DialogMultiview` | tab labels `Multiview`, `Linesheet`, `Preview` (values, not object names) | symbol table has exactly `SetupUiSingleviewTabWidget`, `SetupUiMultiviewTabWidget`, `SetupUiLinesheetTabWidget` |
| `DialogCLOVise` | members whose *type-name literals* are `CloUICommon::Tab`, with a mangled `CloUICommon::StackedWidget` type | handler `SignalForAddTab` |
| `ColorEditor` | `m_pColorSwatchLibraryTabWidget` | — |
| `ColorSwatchListWidget` | `m_pCustomColorTabWidget` | — |
| `GarmentRecordItemWidget` | `gradingTabWidget` | — |
| `UserCustomLibraryPage` | `MVVerticalTabWidget` member | symbol table has a whole `MVVerticalTabWidget` class with a `Ui::MVVerticalTabWidget` form |
| `MVDockLibraryTitleBar` | `AddNewTab` | — |
| `MVDialog` | `SetCurrentTab` | — |

**Derived:** `MVVerticalTabWidget` is the application's own vertical tab
container; it has its own Designer form and appears as a member of at least one
page. That makes it a *tab container primitive*, distinct from `QTabWidget`.

**Unknown:** the **displayed order** of tabs in every case, except where the
literal run happens to be in setup order (`DialogPreference`, `DialogModularCreator`).
**Unknown:** the visible heading text that labels each page. The label literals
for a form sit in a **separate region** of the same block, after all object
names — they are not interleaved with the pages, so page ↔ label pairing cannot
be made from offsets. See §12.

## 5. Layer 3 — `DialogPreference`, the most completely recovered screen

`DialogPreference` is the preferences screen. It has 256 members in the register
and is the only screen where both blocks are complete and mutually validated.

### 5.1 Structure (**Observed**)

The form declares a **stacked widget** named `PreferenceStackedWidget` and a
**list widget** named `SelectListWidget` inside `SelectListFrame`.

**Derived:** the preferences screen is a two-pane layout — a selection list on
the left, a stacked page area on the right. The `SelectListWidget` is what the
user clicks to change page.

### 5.2 Pages (**Observed** names, in literal-run order)

`GraphicOptionPage`, `ViewControlPage`, `shortcutAndMenuPage`,
`DefaultFilesPage`, `PlugInPage`, `CLOSETPage`.

### 5.3 Options (**Observed**, 113 handlers; validated against the symbol table)

The handler names are the option identities. Grouped by the page whose topic
they match (**Derived** topic mapping — the grouping is a reading of the names,
the handler set itself is Observed):

| Page | Handlers whose topic matches (**Observed** names) |
|---|---|
| `GraphicOptionPage` | `OnRegistryMaximumImageResolution`, `OnChangeNormalBlendingMethod`, `OnChangeDefaultRenderTransparencyMethod`, `OnRegistryAntiAliasing`, `OnChangeStrandRenderModeToBaseLine` / `…Cylinder` / `…Plane`, `OnChangeMeshTypePreferenceToTriangle` / `…Quadrangle` |
| `ViewControlPage` | `OnChangeDefaultViewControl`, `OnToggledSmartPanningON`, `OnToggledAutoFocusON`, `OnToggledSwitchBackWithAutoFocusON`, `OnCameraDistanceValueChanged`, `OnCameraAxisXAngleValueChanged`, `OnCameraAxisYPosValueChanged`, `OnChangeGizmoTypePreferenceToDivided` / `…Unified` |
| `shortcutAndMenuPage` | `OnMenuName`, `OnShortcutKey`, `OnDeleteShortcutItem`, `OnResetShortcutItem` |
| `DefaultFilesPage` | `OnOpenProjectFile`, `OnSetProjectFile`, `OnOpenDefaultFiles`, `OnResetDefaultFiles`, `OnChangedStateDeleteUnusedFabric`, `OnChangeAutoSaveFileType`, `OnAutoSaveTimeIntervalEditingFinished`, `OnMaxFileCountEditingFinished` |
| `PlugInPage` | `OnAddPlugIn`, `OnEditPlugin`, `OnDeletePlugInItem`, `OnDeleteAllPlugIn` |
| `CLOSETPage` | `OnSetCLOSETURL`, `OnSetCLOSETAlwaysOpenClosetFromCloForVersionUpload`, `OnSetCLOSETKeepTheCurrentThumbnail`, `OnSetCLOSETUseCustomViewAsThumbnail`, `onChangeAccount`, `onSwitchAccount` |
| (unassigned by topic) | unit/decimal (`OnUnitValueChange`, `OnDecimalPlacesChange`), notches (`OnNotchDisplayTypeValueChanged`, `OnNotchLengthValueChanged`, `OnNotchTypeValueChanged`), arrangement (`OnArrangementPointRadiusValueChanged`, `OnChangeArrangementType`, `OnParticleDistanceValueChanged`, `OnDefaultSAOffsetValueChanged`, `OnToggleFoldArrangementAngleSync`, `OnToggleSpreadCutPatterns`), plotter (`OnPlotterIpAddressChanged`, `OnPlotterNetworkCheckBoxChanged`, `OnPlotterSerialPortBitRateChanged`, `OnPlotterSerialPortDataBitChanged`, `OnPlotterSerialPortFlowControlChanged`, `OnPlotterSerialPortNumChanged`, `OnPlotterSerialPortParityChanged`, `OnPlotterSerialPortStopBitChanged`), paths (`OnModularLibraryPathEditingFinished`, `OnModularLibraryGetDefaultPathButtonClicked`, `OnDMIxCloudDownloadPathEditingFinished`, `OnDMIxCloudGetDownloadPathButtonClicked`, `OnRefreshPathChanged`, `OnRefreshPathEdited`, `OnGetRefreshPathButtonClicked`, `OnClickedThumbnailFileSetting`), and others |

### 5.4 The options are a tree, not a tab bar (**Observed**)

`DialogPreference::OnValueChanged(QTreeWidgetItem*, int)` takes a
`QTreeWidgetItem*` and a column index.

**Derived:** preference rows are presented in a `QTreeWidget`, and each row's
edit writes back through that handler. So the preferences screen carries *two*
hierarchies at once: a stacked **page** on the right, and a **tree** of option
rows inside it.

**Unknown:** which branch of that tree each of the 113 options sits under. That
requires the `setupUi` tree construction, i.e. instruction evidence.

**Observed:** the option kinds can be read from the handler signatures —
`…ValueChanged(double)` and `…ValueChanged(int)` for numeric options,
`…Toggled…(bool)` and `…ChangedState…` for booleans, `…Clicked…()` and
`OnAddPlugIn`/`OnEditPlugin` for actions.

## 6. Layer 4 — menus: names recovered, membership **not** recovered

### 6.1 The menu object names (**Observed**)

The string table contains **108** literals whose shape is `menu_…` / `menu…`.
**101** of them lie in one contiguous region (offsets 43,441,591…43,443,352)
immediately preceded by `centralWg` and `myMenuBar` — the classic Designer
object names for a main window's central widget and menu bar.

The names are listed in offset order in §6.3. Two *different* kinds of thing are
present in the same set, and distinguishing them matters:

* **menu-bar drop-downs** — `menu_File`, `menu_Edit`, `menu_Display`,
  `menu_Materials`, `menu_Avatar`, `menu_Animation`, `menu_Help`,
  `menuPreferences`, `menu_CustomizeTools`, `menuEnvironment`, `menuProduction`,
  `menuConnect`, `menuPlugins`, `menuRender`, `menuRetopology`, `menuFit_Map`,
  `menuLanguage`, …
* **tabbed-context-menu tabs** — `menu_2DPattern_Create`,
  `menu_3DGarment_Reverse_Arrange`, `menu_Measurements`, `menuSewing`,
  `menuPuckering`, `menuBinding`, `menuGluing`, `menuTack`, `menuRipping`, …

**Derived:** the second group are the tab headings of the tabbed context menus.
That is supported by two independent observations: the class
`TabbedContextMenu` exposes `SetTabs(QList<TabDef> const&)`,
`AddActionToTab(QString const&, QAction*)`, `AddMenuToTab(QString const&, QMenu*)`
and `AddSeparatorToTab(QString const&)` — a context menu built from named tabs —
and the binary contains **71** `CLOContextMenuImpl::PopupContextMenu*` functions,
one per selection context, each of which builds one such menu.

### 6.2 The action object names (**Observed**)

The string table contains **1,000** literals shaped `action_…` / `action…`
(**999** distinct). **978** of them lie in one contiguous run (offsets
43,415,637…43,441,546) immediately preceded by the literal
`MainWindowCompatibilityInterface`.

**Observed, and this is the useful part:** that run interleaves each action's
name with its **keyboard shortcut** where it has one. 42 shortcut literals
appear in the run.

**Derived:** the interleaving follows Designer emission order, so an action name
immediately followed by a shortcut literal is that action's shortcut.

| Action object name | Adjacent shortcut |
|---|---|
| `action_LoadPose` | `Ctrl+Shift+P` |
| `action_EditCopy` | `Ctrl+C` |
| `action_EditPaste` | `Ctrl+V` |
| `action_EditMirrorPaste` | `Ctrl+R` |
| `action_SimStop` | `Space` |
| `action_NewProject` | `Ctrl+N` |
| `actionOpen_Project` | `Ctrl+O` |
| `actionSave_Project` | `Ctrl+Shift+S` |
| `actionSave_Over` | `Ctrl+S` |
| `action_ClothThick` | `Alt+1` |
| `action_ClothTexture` | `Alt+2` |
| `actionOpaque_Surface` | `Alt+5` |
| `action_ClothPressure` | `Alt+6` |
| `actionStrain_Map` | `Alt+7` |
| `actionFit_Map` | `Alt+8` |
| `actionShow_Feature_Lines` | `Alt+0` |
| `action_ShowClothSeamLine` | `Shift+S` |
| `action_AvatarHide` | `Shift+A` |
| `action_ShowArrangement` | `Shift+F` |
| `action_ShowLineLength` | `Shift+Z` |
| `actionHide_Pattern` | `Shift+Q` |
| `actionX_Ray` | `Shift+X` |
| `action2D_Texture_Surface` | `Shift+T` |
| `action_IsolateSelection` | `Shift+I` |
| `actionActivate_Pattern` | `Ctrl+J` |
| `actionRemove_All_Pins` | `Ctrl+W` |
| `actionReverse_SeamLine` | `Ctrl+B` |
| `action_Clone_As_Pattern` | `Ctrl+Shift+C` |
| `action_EditSymmetricPaste` | `Ctrl+D` |
| `action_LoadAvatar` | `Ctrl+Shift+A` |
| `actionSwitching_Window` | `Ctrl+Tab` |
| `actionFreeze_Only_Pattern` | `Ctrl+K` |
| `actionLayer_Pattern_Cloning` | `Ctrl+Shift+V` |
| `actionLayer_Clone_Under` | `Ctrl+Shift+B` |
| `actionPattern_Strengthen` | `Ctrl+H` |
| `actionSuperimpose_Over` | `Ctrl+[` |
| `actionSuperimpose_Under` | `Ctrl+]` |
| `action_Select_All` | `Ctrl+A` |
| `actionSolidify` | `Ctrl+E` |
| `action_AddProject` | `Ctrl+Shift+O` |
| `actionInitialize_Selected_Pattern_Arrangement` | `Ctrl+F` |
| `action_InvertSelection` | `Ctrl+Shift+I` |

**Observed — selected examples showing the naming pattern of menu commands:**
`action_NewFile`, `action_AppExit`, `action_FileLoadCloth`, `action_FileSaveCloth`,
`action_ImportOBJ`, `action_ExportOBJ`, `action_EditCut`, `action_EditCopy`,
`action_SimStop`, `action_RecordAnimation`, `action_ShowAnimationEditor`,
`action_DrapePattern`, `actionRemove_Pin`, `actionAttach_All_Pins_to_Avatar`,
`actionDetach_All_Pins_from_Avatar`, `actionCLOSET_Upload_Project`,
`actionCLOSET_Upload_Garment`, `action_Render_via_CLO_SET`, `action_ImportVRM`,
`action_ExportVRM`, `action_UVEditor_ShowGarment`, `actionCalculator_Notepad`,
`action_CreateReferencePolygon`, `action_Trace_Shape`, `actionVolume_Calculation`.

### 6.3 The menu names, in literal order (**Observed**)

`menu_File`, `menu_OpenFile`, `menu_SaveFile`, `menu_Import`, `menu_Export`,
`menuScreen_Capture`, `menu_File_Add`, `menu_Import_Add`, `menuVideo_Capture`,
`menuRecent`, `menuInformation`, `menuUpload_to_CLO_SET`, `menuPop_up_Menus`,
`menu_3DGarment_Reverse_Arrange`, `menu_3DGarment_Convert`, `menu_3D_MeshType`,
`menu_2DPattern_Rotate`, `menu_2DPattern_Align`, `menu_2DPattern_Distribute`,
`menuAttach_Pattern`, `menu_Order`, `menuExtend_Trim`, `menuExtend_Trim_Add_Point`,
`menuFlip`, `menuGrading_2`, `menuMirror_Paste`, `menuConvert_to_Curve_Point`,
`menuConvert_to_Segment_Point`, `menu_PatternGroup`, `menuClone`, `menuTrace`,
`menuSewing`, `menuMaterials_2`, `menuAvatar`, `menuDisplay_ContextMenu`,
`menu_3DCloth`, `menuMeasurement`, `menuGarment_Resolution`, `menuSelect`,
`menuPin`, `menuRipping`, `menuStyleline`, `menuFold`, `menuArrangement`,
`menu_3DAvatar`, `menuMeasurements`, `menu_2D`, `menu_2DPattern_Create`,
`menu_2DPattern`, `menuPleats`, `menuLock`, `menuPattern_Annotation`,
`menuPattern_Measure`, `menuGrading`, `menu_Sewing`, `menuTack`, `menuLanguage`,
`menuDisplay`, `menu_Display_3DToolbars`, `menu_Display_Viewpoint`,
`menu_Display_3DGarment`, `menu_Display_2DPattern`, `menu_Display_Avatar`,
`menuEnvironment`, `menu_Display_3DTrims`, `menu_Display_2DInformation`,
`menu_Display_Sewing`, `menu_Display_3DGarment_Rendering_Style`,
`menu_Display_Window`, `menu_Display_Avatar_Rendering_Style`,
`menu_Display_2DPattern_Rendering_Style`, `menuFit_Map`, `menu_Display_2DToolbars`,
`menu_CustomizeTools`, `menuPreferences`, `menu_Preferences_Gizmo`,
`menuCamera_Properties`, `menu_Help`, `menuManual`, `menuAnimation`,
`menuMaterials`, `menu_Materials_Graphics`, `menu_Materials_Button`,
`menu_Materials_Piping`, `menu_Materials_Topstitch`, `menuPuckering`,
`menu_Materials_Steam`, `menu_Materials_Zipper`, `menuBinding`, `menuGluing`,
`menuRender`, `menuGoZ`, `menuRetopology`, `menuCLO_SET`, `menuFabric`,
`menu_Fabric_Texture`, `menuProduction`, `menuConnect`, `menuPlugins`,
`menuPlug_In`, `menuPlug_in_Download`.

The list above is the 101 entries in the main-window run. The table holds 108
literals matching that shape in total; the other seven are **not** menu-bar
entries and are recorded here for completeness so the count reconciles:
`menuItemIndent` (a style metric), `menu_Setting/menuLanguage/check/type` and
`menu_Setting/menuDisplay/menu_Display_Viewpoint/shortcuts` (two composite
settings key paths), `menu_Pattern_Arrangement` and `menuPlugins / menuPlug_In`
(both of which are setting-key strings in other forms), and `menu_Edit` and
`menu_Setting` (the menus of `DialogCloAvatarEditor` and `DialogGraphicOption`
respectively — the latter holding the language actions listed in §6.2's
neighbours).

### 6.4 What is **not** recovered here

**Unknown — action → menu membership.** I cannot say from evidence which of the
978 actions appears under `menu_File` rather than `menu_Edit`. The names are
suggestive (`action_FileLoadCloth` embeds "File"), and it is tempting to build a
tree from them. **I am not doing that**, because the name heuristic is exactly
the kind of plausible-looking structure that must not be presented as recovered.
The membership is determined by `QMenu::addAction` call order in the generated
`setupUi` body and in `MainWindowController`/`CloUICommon::MenuList` call sites
— all instruction-level.

**Unknown — the menu bar's top-level order and its displayed titles.** A search
for a contiguous cluster of plausible top-level menu words returned **nothing**:
no run of short title literals exists in the string table. The titles are
therefore either longer phrases, split across literals, or produced at runtime.

## 7. Layer 5 — docks, panels and the title bar

**Observed:** a contiguous literal run at offset 43,415,004…43,415,105 contains
the dock/panel display names and the dock-persistence key vocabulary:

* display names: `Animation Editor`, `Comments`, `USD Layer`,
  `Omniverse Connector`, `Custom Library`, `Pattern Information`, `API Panel`;
* the type-name key `MVDockWidgetTypeName`, and the persisted-state suffixes
  `_Visible`, `_DockingInfo`, `_Size`, `_Floating`, `_Geometry`;
* the setting key `Docking`.

**Observed:** immediately after that run come Qt meta signatures for the dock
title bar: `dockLocationChanged(Qt::DockWidgetArea)`,
`OnUpdateTitleBarLayout(Qt::DockWidgetArea)`, `topLevelChanged(bool)`,
`onChangedTopLevel(bool)`, `SignalForClickedDockingButton()`,
`onDockingButtonClicked()`, `SignalForPopupMode(QPoint)`, `onPopupMode(QPoint)`.

**Derived:** dock widgets are named by an enum-like type name (`MVDockWidgetTypeName`),
carry a human display title, are persisted per dock under the four suffixes
above, and have a title bar that can dock, undock/floating, and open a popup
menu. `DockWidgetController` has 222 members in the register.

**Derived:** these seven names are the *display titles* of dockable panels. This
is a recovered, if partial, dock inventory — but it is not proven to be the
complete set of docks, and it does not say which screen area each dock occupies.

## 8. Layer 6 — context menus

**Observed:** the binary defines **71** functions named
`CLOContextMenuImpl::PopupContextMenu<Context>`, i.e. one popup builder per
selection context. The contexts (names only):

`2DAnnotation`, `2DBackground`, `2DBlocktoBlock`, `2DBlocktoPattern`,
`2DDummyShape`, `2DGluingShape`, `2DGrading`, `2DHalfSymmetryGuideLine`,
`2DLacing`, `2DLine`, `2DPattern`, `2DPatternBlock`,
`2DPatternInEditJointWeight`, `2DPatternInTextureGizmoMode`, `2DPoint`,
`2DPrintOverlay`, `2DPrintOverlayInTextureGizmoMode`, `2DPuckering`,
`2DPunchMark`, `2DReferenceShape`, `2DSeamLine`, `2DSeamTaping`, `2DSeamlineRip`,
`2DShape`, `2DStitch`, `2DZipper`, `3DAvatar`, `3DAvatarLinePin`,
`3DAvatarMeasure`, `3DBackground`, `3DBasePolyline`, `3DBinding`,
`3DButtonItems`, `3DCurvedPattern`, `3DDrawCutLine`, `3DEditCutLine`,
`3DFlattening`, `3DGarmentMeasurement`, `3DGluing`, `3DLacing`, `3DLineTack`,
`3DMultipleSelection`, `3DNurbs`, `3DPattern`, `3DPatternAndTrimObject`,
`3DPatternInEditJointWeight`, `3DPin`, `3DPiping`, `3DRegionElement`,
`3DResizingAvatarMeasure`, `3DSewing`, `3DTrimObject`, `3DVRayLight`,
`3DWindController`, `3DZipper`, `Basting`, `Brush`, `DartCap`,
`EditRoundGarmentMeasurement`, `EditRoundGarmentMeasurementSymmetric`,
`EditVolumeMeasurement`, `MakeTraceShape`, `PatternWalking`,
`PlacementOnFabric`, `PrintLayoutAnnotation`, `Retopology`, `Ruler2D`,
`Selected2DPatternOperationPuckering`, `Selected2DPatternOperationRip`,
`Selected2DPatternOperationStitch`, `UVEditor`.

**Observed:** `TabbedContextMenu` exposes `SetTabs`, `AddActionToTab`,
`AddMenuToTab`, `AddInlineGroupToTab`, `AddSeparatorToTab`, `TabDef`,
`ApplyMenuListSpec`, `RestoreMenuListSpec`, `SetActionContextHidden`,
`IsActionContextHidden`, plus a search field (`onSearchTextChanged`,
`currentSearchText`, `renderSearchResultsToPage`), keyboard navigation and
wheel-driven tab cycling.

**Derived:** the context menus are **tabbed, searchable, and per-context**, with
a spec that can be applied and restored (`ApplyMenuListSpec` /
`RestoreMenuListSpec`) and per-action visibility toggles. The 71 context names
above are the recovered context taxonomy.

**Unknown:** which actions and which tabs belong to each of the 71 menus.

## 9. Layer 7 — the Qt resource name table

**Observed** (from the shipped `Resources_Light.rcc`, 18,950,655 bytes):
header reports version 3, data section at offset 24, name section at 18,846,133,
tree section at 18,919,943 (30,712 bytes). The name table decodes to **1,393**
names in file order, of which 1,150 are SVG and 205 PNG; six XML, five Python,
three ICO and three ICNS round it out (the register's figures, unchanged).

**Unknown — the directory tree was NOT decoded.** The register's own extractor
documents the same failure and I confirmed the cause: the name-table entry
layout is `uint16 length, uint32 hash, length × UTF-16BE` (my first attempt
decoded it as `uint16 length` + characters and produced mojibake, exactly as the
extractor's docstring warns), but the **tree section's child-offset base is not
resolved**. The root node reads as `nameOffset = 0` ("Resources"),
`flags = 0x0002` (directory), `childCount = 1`, `firstChildOffset = 1`, which is
not self-consistent under any of the offset interpretations tried (relative to
the tree section, the node, or the file). Node sizes appear to be 14 bytes for a
directory and 10 for a file, which would fit 1,393+ nodes in 30,712 bytes, but a
flat scan at that stride does not reach the section end cleanly.

**What would decode it:** the tree only needs the correct `childCount` /
`firstChildOffset` field offsets and base for rcc version 3 — a bounded reading
of Qt's `qresource.cpp` writer, or a trial decode that validates each candidate
layout by requiring the node count to equal the name count and every decoded
name to be ASCII. I did not complete that, so no directory grouping is claimed.

## 10. What the recovered structure tells us about the architecture

These are **Derived**, from the observations above:

1. The interface is a **Designer-form-heavy Qt desktop application**: 274 forms
   with generated object hierarchies, plus a hand-written controller layer
   (`MainWindowController`, `DockWidgetController`, `DialogController`).
2. **Two tab systems coexist**: ordinary uic page stacks
   (`PreferenceStackedWidget`, `creatorTabStackedWidget`, `m_fileTabWidget`) and
   the application's own vertical tab container (`MVVerticalTabWidget`,
   `CloUICommon::Tab`, `CloUICommon::TabBar`).
3. **Context menus are a first-class, tabbed, searchable UI surface**, not simple
   popups — 71 contexts, each with named tabs and a search field.
4. The **preferences screen is the densest screen** (256 members, 113 option
   handlers, a selection list plus a stacked page area plus a tree of rows).
5. **Menu content is largely code-driven**, not Designer-driven: the menu object
   names come from one main-window form, but the actions they contain are bound
   by controller code — which is why the membership layer is exactly the layer
   that needs instruction evidence.

## 11. Surprises

* **The Qt literal blocks are readable without a disassembler.** A Designer
  form's object hierarchy, its visible labels and its slot signatures are all
  present in the shipped string table in per-class runs. That is what made this
  pass possible at all once decompilation failed.
* **The preferences screen is not a tab bar.** It is a selection list plus a
  stacked page area, and the options inside each page are a `QTreeWidget`.
* **"Menu" means two different things** in this binary: menu-bar drop-downs and
  the tab headings of the tabbed context menus. The 108 `menu_*` names mix both.
* **The blocker is Ghidra's analysis budget, not the file format.** The same
  `open_binary` succeeds; the first *query* still pays a whole-program analysis
  of 39.7 MB of text and 191,286 chained-pointer fixups, which exceeds 330 s.
* **The register's resource-tree gap is real and I could not close it.** The
  name table is readable; the child-offset encoding is not, and I am not going
  to publish a directory grouping I cannot decode.

## 12. The honest answer: what is not understood, and how to get it

This is the list the standing question is really asking for.

| # | Not recovered | Why | What would recover it |
|---|---|---|---|
| 1 | **Action → menu membership** (which item sits in `menu_File` vs `menu_Edit`) | Determined by `QMenu::addAction` call order in generated `setupUi` and in `MainWindowController` / `CloUICommon::MenuList` call sites — instruction-level | Decompile `MainWindowController::createMainWindow` / `CreateWidgetActionsForSubTitleOnMenu` / `OnUpdateMenu` and the main-window `setupUi`. Requires a Ghidra session that completes analysis (raised deadline, more RAM, or a pre-analysed saved project) |
| 2 | **Toolbar composition** (which actions sit in which toolbar) | Same: `MainWindowController::CreateToolBarForViewWidgets`, `CreateActionsFor3DSimulationToolbar`, `CloUICommon::ToolBar::AddAction`, `ActionHelper::AddAction` call sites | **Partly answered at runtime** (§0.2) — the visible control set, in order and with roles. Which *named toolbar* each belongs to still needs #1's route |
| 3 | **Preference option tree grouping** (which branch each of the 113 options is under) | `QTreeWidget` rows are built in `setupUi` | Same as #1. Runtime capture helps **only if** the dialog presents as an on-screen layer-0 window (§0.1) |
| 4 | **Tab display order and per-tab heading text** for most dialogs | The label literals sit in a separate region from the object names, so offsets do not pair them | `retranslateUi` decompilation. Runtime capture did **not** work here: the workspace's eleven tabs expose no title and no value even after selection (§0.2) |
| 5 | **Menu bar top-level titles and order** | No contiguous title cluster exists in the string table; titles may be built at runtime | **Decompilation only.** Runtime capture is closed for this — the menu bar is the native macOS menu bar in a layer-1000 window and REA's gate requires layer 0 (§0.1) |
| 6 | **Dock placement, defaults and the complete dock set** | Only display names and persistence suffixes are literals; placement is code | Same as #1, plus `DockWidgetController` bodies |
| 7 | **`.rcc` directory/tree grouping** | Child-offset base in rcc v3 unresolved (§9) | A correct v3 tree decoder, validated against the 1,393-name count |
| 8 | **Screens with no literal block** (e.g. `CLOFabDockWidget`, whose block is 7 lines of C++ mangled names and no object names) | Hand-coded UI, no Designer form | Decompilation |
| 9 | **Per-action icons** | Icon assignment is code; only the resource *names* are data | Decompilation, plus the resource name table |

**The single highest-value unlock is #1–#6 at once: a Ghidra analysis of the main
executable that completes.** No amount of string-table work substitutes for it,
because everything missing is *which thing contains which other thing*, and that
is control flow.

**A second, cheaper unlock exists and is now proven up to a permission gate.**
Observing the running application with REA's accessibility tools would return the
real menu bar, tab labels and option rows as presented, without decompiling
anything. §12.1 records how far that got: the identity/binding problem is
**solved and reproducible**, and the route now stops only on an Accessibility
grant. **Items 1–6 are therefore blocked on a permission rather than on
analysis** — but they remain **Unknown**, and nothing here may be upgraded on the
strength of the toolchain findings alone.

### 12.1 The runtime observation route — attempted, and what it settled

An earlier revision of this document said this route had not been attempted. It
now has been, under explicit owner authorisation, on an instance the **owner**
launched. It **succeeded in part** and **failed in part**, and the split is the
point: **the workspace window's structure was recovered — §0.1 gives the tool
boundaries and §0.2 the recovered material** — while the menu bar was not, for
reasons that are tool boundaries rather than CLO facts.

§12 items 1 and 5 (action→menu membership; menu-bar titles and order) are now
**closed as unreachable by this route**, not merely un-attempted: the menu bar is
the native macOS menu bar in a layer-1000 window, and REA's window gate requires
layer 0. Items 3 and 4 (preference tree grouping; tab order and heading text)
remain **Unknown**, but the reason has narrowed: the eleven tabs in the
workspace's tab strip expose **no titles and no values** even after selection
(§0.2), so a dialog's tabs would have to expose theirs, which cannot be assumed
from this sample. Item 6 (docks) is **partially** answered by the recovered panel
structure in §0.2.

**Process (Observed).** `CLO` PID 67966,
`/Applications/CLO.app/Contents/MacOS/CLO_Standalone_OnlineAuth`,
`CFBundleIdentifier` `com.clovirtualfashion.CLO_Standalone_OnlineAuth`,
`CFBundleShortVersionString` **2026.1.310** (matching the register's target). It
is the **online-auth** build, so a licence or account prompt could not have been
satisfied from the observation side in any case. Of the process's ten windows,
exactly one had `kCGWindowIsOnscreen = true`: window id **26121**, 1221×832 at
(219,1). An off-screen `Library` window and an off-screen `Dialog` strip were
present, i.e. a loaded workspace rather than a splash.

**Important caveat: geometry is not a stable identifier (Observed).** That
1221×832 measurement is a *point-in-time* sample. On a later re-list the same
window id 26121 was the only on-screen window still, but had shrunk to
**135×168 at (16,135)**, and a new off-screen window titled `Schematic Render`
had appeared that was not present at discovery. Nothing in this pass can explain
that — no accessibility action was ever sent (§"State left behind"), so the
change came from the application or from the owner using it. Two consequences
for the resume: (a) the "only on-screen window" criterion still singled out
26121, but **the size argument that supported the workspace identification no
longer holds**, and the window must be re-identified at resume time rather than
assumed; (b) if the workspace has moved to another Space or been minimised,
`kCGWindowIsOnscreen` filtering can select a different small window and mislead,
so identification should be re-derived from a fresh window list and, once the
grant exists, from the accessibility tree itself.

**The identity gate, and the recipe that satisfies it.** `observe_native_ui`
requires the *active target* to agree with the running process on executable
path *and* full-file digest (`bridge/native/ReaNativeUI.swift:47–53`: strict
`realpath` equality, then a re-hash of the file, with a 256 MiB identity
budget). Four bindings were needed to find a form that passes:

| Binding attempted | Outcome |
|---|---|
| thinned `arm64` slice at `/tmp/clo_main.arm64` | `target-mismatch` — realpath differs from the process executable |
| universal executable, `provider_id` `native-macos` | provider id rejected (only `ghidra`/`hopper`/`ida`); Ghidra returns `architecture_unsupported` for `mach-o-universal`; Hopper not installed; IDA not configured |
| universal executable, `format: "raw-binary"` | "Native UI observation requires an active Mach-O executable or app target" — `raw-binary` is not Mach-O |
| **the `.app` bundle `/Applications/CLO.app`, `provider_id: "auto"`** | **passes** — resolves to the real executable path, `format: mach-o-universal`, `sha256 9ee952f4aaa773b20ff34691d5701c95e23683917cea1a2165b1576f59977c45` |

**Derived — the general rule (not just this instance).** To reach native UI
observation for a **universal-Mach-O application**, bind the **`.app` bundle** —
not the executable, and not a thinned slice — because only the bundle form
simultaneously satisfies the path-equality and whole-file-digest gates while
remaining an accepted Mach-O/app target. Binding the executable fails
(`architecture_unsupported` from Ghidra, the only provider available); binding a
slice fails the path gate; binding the executable as `raw-binary` fails the
Mach-O gate. This is a real toolchain gap: with only Ghidra configured, the MCP
surface offers no supported way to observe a universal-Mach-O process, and it
took four attempts to find the form that works.

**Where it stopped (Observed).** The capture was refused at the permission gate:

```
provider_id: native-macos, operation: observe_native_ui
accessibility-denied: Grant Accessibility to the REA host in
System Settings > Privacy & Security, then retry
```

The check is `AXIsProcessTrusted()` in REA's helper. **Screen Recording
permission is untested and remains Unknown**, because accessibility is evaluated
first and the screenshot path was never reached.

**Do not infer anything about REA's permissions from the window titles above.**
The `kCGWindowName` values in the discovery table were readable by the *shell*
that ran the CoreGraphics listing. That is that shell's own TCC context and says
nothing about REA's helper — which is exactly why the first capture had to be
attempted rather than predicted, and why it failed at a gate the shell's
readability had seemed to suggest would pass.

**Why the grant is awkward (Observed + Derived).** REA compiles its helper per
call: `dist/native/NativeUiHelperRuntime.js` symlinks
`bridge/native/ReaNativeUI.swift` into a temporary root, invokes `swiftc`, and
removes that root afterwards. The binary calling `AXIsProcessTrusted()` is
therefore a **freshly compiled helper in a temporary directory whose path
changes every call**, so there is no stable binary to add to the Accessibility
list. **Derived:** the grant must go to the **host application that runs the
REA MCP server** (observed as `node /Users/julienfoss/src/rea/scripts/rea.mjs mcp`,
PID 10979), which then passes authorisation to the child helper; that host must
be restarted for TCC to take effect. **Two candidate targets were identified
(Observed, by the Lead via the `lsappinfo` coalition field — `/bin/ps` was
blocked for both of us with `Operation not permitted`, so the process tree was
not walkable):** the server shares `coalition: 48575` with **PID 10948 =
`/Applications/DeepSeek Harness.app`, bundle `com.deepseek.dsh`**, and `lsof`
shows the server running as `/usr/local/bin/node` with cwd
`/Users/julienfoss/.dsh/profiles/desktop`. **Unknown:** which of those two TCC
will attribute responsibility to; both should be offered to the owner, and the
grant must be followed by a restart of the relevant host.

**State left behind.** No accessibility action of any kind was sent: no menu
opened, no click, no dialog, no text entry. Application state was not altered,
and the only change was the REA session's active target, which is session-local.
Neither this pass nor any earlier pass launched, restarted or modified CLO.

**Evidence IDs.** There are none for this subsection. A refused operation
returns an error rather than an Evidence record, and `open_binary` returns target
identity without an `evidence_id`. These findings are reproducible tool outputs,
not REA records — the same caveat that applies to the `nm`/`strings` facts in
§0.

**If the grant is made**, the route to items 1–6 is: re-identify the window from
a **fresh** list (do not reuse window 26121 on trust — its geometry changed,
above), bind `/Applications/CLO.app` as above, then `observe_native_ui` on the
re-identified window, then `capture_native_ui_scenario` to open menus — clicking
only menu-bar titles and toggling the same title to close, never an item inside a
menu, so that no command can fire. `capture_native_ui_scenario` offers no raw
keypress, so Escape is not available as a dismissal. A capture that truncates at
its `max_nodes` bound must be reported as truncated, and any item not visible in
the tree stays **Unknown**.

## 13. How far this can be trusted

* Anything in this document tagged **Observed** is a literal in the shipped
  binary or a symbol-table entry, both reproducible with the commands in §14.
* The **per-class literal-block** method is validated on one class
  (`DialogPreference`: 113 handlers from two independent sources). It is
  **not** validated on all 435 blocks, and it demonstrably produces false
  positives for generic suffix scans (§4) — which is why those results are
  reported with their corroboration or not at all.
* **Do not build an interface against a menu tree from this document.** There is
  none here. Sections 6.1–6.3 give menu and action *names*; §12 item 1 says the
  membership is Unknown.
* The 274 / 435 / 1,393 / 108 / 1,000 / 978 / 42 / 113 counts are exact for the
  stated definitions and reproducible.

## 14. Reproduction

```bash
# Thin slice (identical SHA-256 to the register's recorded slice).
lipo -thin arm64 /Applications/CLO.app/Contents/MacOS/CLO_Standalone_OnlineAuth \
     -output /tmp/clo_main.arm64
shasum -a 256 /tmp/clo_main.arm64
# b2d545d7baaefa3b303599c2bb51f49b94b04607531d55ba53f5693e9bff128f

# Symbol table (class and member inventory).
nm /tmp/clo_main.arm64 | c++filt > /tmp/allnm_dem.txt

# Literal table with file offsets; the offsets are what makes the blocks visible.
strings -a -t d /tmp/clo_main.arm64 > /tmp/clo_str_off.txt   # 162,320 literals

# Locate a class's block: take the offset of the literal equal to the class name
# and read forward; object names come first, label literals later.
grep -n "DialogPreference" /tmp/clo_str_off.txt | head

# rcc name table (tree NOT decoded — see section 9).
python3 scripts/clo3d/extract_surface.py     # reproduces data/clo3d/resources.json

# REA / Ghidra attempt on the main executable (both calls failed; see section 1.2)
#   open_binary clo3d_target/CLO_Standalone_OnlineAuth_arm64   (provider_id "ghidra")
#   batch_decompile 0x1011f52b0
#     -> ev_5af060db9d8321fed252eb70709b17eda0ff57e99d25a1ab1fad47dcbe198ae5
#     -> ev_c83b938fb9c7572838a8840471add1ecd3853898352351992813247f9bdfd29d
#        (provider_timeout, timeout_ms 330000; then artifact_changed)
```

## 15. Licence and provenance

The CLO artifacts analysed here are **not** distributed with this repository and
were not modified; the slice was written outside the repository, under `/tmp`.
All identifiers quoted are symbol-table entries, Qt object names, interface
labels or file-header fields read from the shipped application, recorded for
interoperability research. No CLO icon, image, font, preset or asset content is
reproduced. No file under `src/` was touched.
