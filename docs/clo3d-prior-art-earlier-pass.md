# ClothCraft — Prior-Art & Feasibility Survey

**Date:** 2026-10-15
**Purpose:** Survey existing open-source work toward a CLO3D / Marvelous Designer
equivalent, before any reverse-engineering or Rust reimplementation begins.

> All repository metadata below was retrieved live from the GitHub REST API.
> Star counts and activity dates are as of the date above.

---

## 1. Executive summary

There is **no single open-source project that covers the full CLO3D workflow.**
The problem decomposes into six largely independent pillars, and each pillar has
one or two credible open-source incumbents — but nothing stitches them together
into a garment-design application.

The closest full-pipeline attempts are **academic research code** (Maria
Korosteleva's GarmentCode line at ETH Zurich), not product-grade applications.
The closest *application* is a zero-install browser tool (Prenda3D-Lab). The
established *patternmaking* tool is Seamly2D, which has a 2D drafting engine but
no simulation.

**Implication:** a Rust reimplementation should integrate existing open-source
libraries per pillar rather than attempt a monolith — and the reverse-engineering
effort should target the *format interoperability layer* (CLO's `.zprj`/`.zpac`,
Marvelous `.zprj`/`.avt`/`.obj`) where the real lock-in lives, not the solver
internals, which are published in the academic literature anyway.

---

## 2. Pillar-by-pillar prior art

### 2.1 Sewing-pattern representation & generation

| Project | Stars | Lang | Licence | Notes |
|---|---|---|---|---|
| [Garment-Pattern-Generator](https://github.com/maria-korosteleva/Garment-Pattern-Generator) | 274 | Python | MIT | **The reference open implementation.** "Generating Datasets of 3D Garments with Sewing Patterns" (SIGGRAPH 2021). Produces parametric sewing patterns + simulated 3D meshes. The canonical data model to interoperate with. |
| [Garment-Pattern-Estimation](https://github.com/maria-korosteleva/Garment-Pattern-Estimation) | 183 | Python | MIT | Inverse problem: reconstructing sewing patterns from 3D point clouds. |
| [NvidiaWarp-GarmentCode](https://github.com/maria-korosteleva/NvidiaWarp-GarmentCode) | 75 | Python | NOASSERTION | GarmentCode extended for GPU cloth simulation on NVIDIA Warp. **Directly relevant solver reference.** |
| [GarmentCode](https://github.com/maria-korosteleva/GarmentCode) | — | Python | — | The "code-to-garment" DSL: garments described programmatically, then materialised as patterns. |
| [GarmentDiffusion](https://github.com/Shenfu-Research/GarmentDiffusion) | 60 | JS | none stated | Generative model producing sewing patterns. |
| [Seamly2D](https://github.com/FashionFreedom/Seamly2D) | 995 | C++ | **GPL-3.0** | **The mature open patternmaking CAD.** Parametric drafting, `.val`/`.vit` formats, DXF/HPGL export. Actively maintained (Oct 2026). GPL is a hard constraint on code reuse. |
| [plt-pattern-viewer](https://github.com/Dsimpson-man/plt-pattern-viewer) | 0 | HTML/JS | MIT | Zero-dependency HPGL/PLT pattern viewer; parses pieces, seam allowances. Useful reference for the *import* path and for HPGL handling. |

### 2.2 Cloth simulation

| Project | Stars | Lang | Licence | Notes |
|---|---|---|---|---|
| [NVIDIA/warp](https://github.com/NVIDIA/warp) | 7195 | Python/CUDA | Apache-2.0 | GPU simulation framework; the substrate for the GarmentCode work. |
| [stitchMeshing](https://github.com/kuiwuchn/stitchMeshing) | 117 | C++ | — | Stitching 2D pattern pieces into a watertight 3D shell using the stitch-mesh algorithm. **The core geometric primitive CLO does when you "sew" pieces.** |
| [torchxpbd](https://github.com/dendenxu/torchxpbd) | 10 | Python | none | XPBD cloth toy on the StVk garment model. |
| [jbargu/cloth-simulation-bevy-rust](https://github.com/jbargu/cloth-simulation-bevy-rust) | 9 | **Rust** | — | Mass-spring cloth in the Bevy engine. |
| [ThomasConrad/gpu-cloth-sim2](https://github.com/ThomasConrad/gpu-cloth-sim2) | 1 | Rust/wgpu | — | **XPBD on GPU with Rust + wgpu compute shaders.** Closest existing Rust architecture to a real solver. |
| [DeharengOlivier/gpu-cloth-simulation](https://github.com/DeharengOlivier/gpu-cloth-simulation) | 1 | Rust/wgpu | — | Real-time GPU cloth, mass-spring, WGSL. |
| Rigid/soft-body reference | — | — | — | [Rapier](https://rapier.rs) and [Avian](https://github.com/Jondolf/avian) are the mature Rust physics stacks; XPBD cloth would sit alongside, not inside, them. |

**Literature (not repos) that matters:** Position-Based Dynamics / XPBD (Müller
et al.), Projective Dynamics, and C-IPC / IPC for robust contact. These are
published algorithms — reimplementing them is legitimate and well-trodden. This
is the key reason the solver is *not* the hard part.

### 2.3 Avatar / body modelling

| Project | Stars | Lang | Licence | Notes |
|---|---|---|---|---|
| [MakeHuman](https://github.com/makehumancommunity/makehuman) | 1593 | Python | AGPL-ish (NOASSERTION) | Parametric human generator. |
| [MPFB2](https://github.com/makehumancommunity/mpfb2) | 636 | Python | NOASSERTION | Human generator as a Blender addon; actively maintained. |
| SMPL / SMPL-X | — | — | **research-only** | Ubiquitous but licence-restricted for commercial use — a trap for a product. |

### 2.4 Blender bridge / integration path

| Project | Stars | Lang | Licence | Notes |
|---|---|---|---|---|
| [cloth_sewing](https://github.com/potocpav/cloth_sewing) | 5 | Python | — | Blender addon for cloth design and sewing. |
| [blender-marvelous-bridge](https://github.com/cestnimaa/blender-marvelous-bridge) | 2 | Python | NOASSERTION | Session-based Blender ↔ Marvelous roundtrip (Alembic). |
| [Blender's own Cloth solver](https://docs.blender.org) | — | C++ | GPL | Mature cloth sim, GPL-2+ — cannot be linked into a non-GPL Rust binary. |

**Note:** Blender's licence (GPL) makes it usable as a *host/companion*, not as a
library to link. The same applies to Seamly2D.

### 2.5 Automation / interop

| Project | Stars | Lang | Licence | Notes |
|---|---|---|---|---|
| [clo3d-mcp](https://github.com/Lilbonner/clo3d-mcp) | 4 | Python | MIT | **Drives CLO3D via its scripting/Python API** — import projects, dress avatars, assign fabrics, simulate, render, export. Proves CLO exposes a usable automation surface. Valuable as an oracle for behavioural comparison. |

### 2.6 Full-pipeline attempts

| Project | Stars | Lang | Licence | Notes |
|---|---|---|---|---|
| [prenda3d-lab](https://github.com/parka-oss/prenda3d-lab) | 0 | Python | MIT | "Open garment simulation from real sewing patterns (Blender cloth, CPU)." Test benches + garment packages. Young (Oct 2026) but the most *product-shaped* attempt. |
| [CrochetTopology](https://github.com/memacfarland/CrochetTopology) | 0 | — | none | CAD + AI ecosystem, 3D scans → crochet/knit/sew patterns. Empty stub. |
| [garment-pattern-lab](https://github.com/theysayheygreg/garment-pattern-lab) | 1 | JS | none | Sketch → first-draft patterns with a sewing-aware pattern graph. |
| [luxoria-ai-fashion-designer](https://github.com/ibrahimkhalilmasud/luxoria-ai-fashion-designer) | 0 | TypeScript | none | AI fashion design + 2D/3D preview + pattern export. |
| Blender procedural sew | — | — | — | [Blender Sewing](https://github.com/potocpav/cloth_sewing) covers basic stitch-together. |

**Also note:** searches surfaced several repositories that are not open source at
all but *pirated commercial-software distribution* — e.g.
`PuffinMattock82/Marvelous-Designer-2026-Pro-Edition` ("Marvelous Designer 2026 —
3D garment simulation…", no source, licence "Other"). These should be treated as
hostile/noise and must not be used as a design input or dependency.

---

## 3. Gap analysis

| CLO3D capability | Best open-source coverage | Gap |
|---|---|---|
| 2D pattern drafting with parametric measurements | Seamly2D (GPL) | Licence-incompatible; would need clean-room |
| Pattern data model + interchange | GarmentCode (MIT) | Good; adopt as internal model |
| Stitch 2D→3D shell assembly | stitchMeshing (C++) | No Rust implementation |
| Cloth solve (XPBD/PBD) | Warp, gpu-cloth-sim2 | Fragmented; no product-grade Rust crate |
| Fabric material model (anisotropy, bending, buckling) | research only | **Large gap.** CLO's fabric presets are a major differentiator |
| Avatar + sizing | MakeHuman (copyleft), SMPL (restricted) | No permissive option |
| GPU renderer with fabric appearance | — | **Large gap** |
| Project file format (`.zprj`) | none | **Total gap** — no public parser |
| UI/UX (the actual product) | none | **Total gap** |
| Grading / pattern-to-marker / DXF-AAMA export | Seamly2D partial | No permissive option |

**The largest genuine gaps are not the solver — they are the file format, the
fabric material system, and the application/UX layer.**

---

## 4. Licensing map — decides the architecture

| Licence | Projects | Consequence |
|---|---|---|
| **MIT** | GarmentCode, Garment-Pattern-*, prenda3d-lab, clo3d-mcp, plt-pattern-viewer | Safe to reuse/port, including into a Rust product. |
| **Apache-2.0** | NVIDIA Warp | Safe, with patent grant. |
| **GPL-3.0** | Seamly2D | **Cannot** be linked into a permissively-licensed product. Usable only as a separate process or via clean-room reimplementation. |
| **GPL-2+** | Blender | Same; host/companion only. |
| **AGPL-ish** | MakeHuman | Same, worse (network clause). |
| **Research-only** | SMPL/SMPL-X | Cannot ship commercially. |

This map strongly suggests: **build ClothCraft on the MIT/Apache lineage
(GarmentCode + Warp-derived algorithms), keep GPL tools as optional external
interop targets, and clean-room anything CLO-specific.**

---

## 5. Legal and ethical framing (must be settled before any RE work)

Reverse-engineering a commercial application is jurisdiction- and
contract-dependent. Three distinct activities need to be separated:

1. **Interoperability research** — inspecting file formats so a user's own data
   can be read. Widely protected (EU Software Directive Art. 6; in the US,
   *Sega v. Accolade*, *Sony v. Connectix*). Defensible.
2. **Behavioural documentation** — observing what the software does and
   describing it. Defensible as a basis for clean-room reimplementation.
3. **Derivative reimplementation** — porting the actual code, or reproducing
   protected expression (UI layout, icons, shaders, preset libraries). **High
   risk.** A "de- and recompile the entire workflow to Rust" that stays faithful
   to CLO's expression is a derivative work, and CLO's EULA almost certainly
   forbids reverse engineering as a contract term regardless of statute.

**Recommended posture:** treat the target as a *behavioural specification only*.
Use a strict clean-room split — one effort documents behaviour and formats, a
separate effort implements in Rust from those documents, with no code copying.
Name it as inspired-by, not a clone, and do not reuse CLO assets, presets, UI
art, or shaders.

---

## 6. Recommended scope for ClothCraft

Given the above, a credible v1 is **not** "all of CLO3D". It is:

1. `clothcraft-pattern` — Rust pattern data model, clean-room, informed by the
   GarmentCode schema + DXF-AAMA/ASTM D6673 and HPGL/PLT import.
2. `clothcraft-stitch` — Rust port of stitch-mesh assembly (2D pieces → 3D shell).
3. `clothcraft-solver` — XPBD/PBD cloth on `wgpu` compute, anisotropic bending.
4. `clothcraft-fabric` — material parameter model + a *newly authored* preset
   library (never copied from CLO).
5. `clothcraft-avatar` — permissively-licensed parametric body.
6. `clothcraft-app` — `bevy`-based UI. This is the longest pole.
7. `clothcraft-io` — the clean-room format layer, including an independent
   `.zprj` reader for user-owned data.

---

## 7. Environment readiness (this machine)

Verified 2026-10-15:

| Requirement | Status |
|---|---|
| GitHub CLI + auth | ✅ `gh` 2.102.0, account `FireflyTheReal`, scopes: `repo`, `workflow`, `gist`, `read:org` |
| `ClothCraft` repo name | ✅ Available (not found on the account) |
| **REA MCP server** | ✅ Live — `rea-agents` 6.4.0, 207 MCP tools, protocol 2026-07-28 |
| **Ghidra** | ✅ **Installed and fully verified** — 12.1.4 at `/Users/julienfoss/src/ghidra_12.1.4_PUBLIC`. Headless (`analyzeHeadless`), arm64 native decompiler, and OpenJDK 21.0.12.1 all pass every REA provider check. Note: not on `PATH` and not in `/Applications`. |
| Hopper | ❌ Launcher missing at `/Applications/Hopper Disassembler.app` (an optional REA provider; Ghidra covers this work) |
| **CLO3D / Marvelous Designer binary** | ❌ **Not installed.** No app bundle, no `~/Library/Application Support` remnant. **This is the sole remaining blocker.** |
| **Rust toolchain** | ⏳ Not present. Homebrew install path is unusable (see below); installing via the official `rustup` script into `~/.cargo`. |
| Xcode CLT | ✅ `/Library/Developer/CommandLineTools` |
| Homebrew | ⚠️ 7.0.9 installed but **`/opt/homebrew` is not writable by this user** — `brew install` fails with a chown/chmod advisory. Needs `sudo chown -R julienfoss /opt/homebrew` or should be avoided. |
| Host | Apple M1, 8 GB RAM, arm64 |
| **Free disk** | ⚠️ **18 GB** available of 460 GB (Data volume 96% full) |

**Remaining blocker:** the reverse-engineering half cannot begin until a CLO3D or
Marvelous Designer artifact exists on this machine. The analysis toolchain itself
is *ready* — Ghidra is installed and REA is bound to it, and both report healthy.
Once a binary is present, `open_binary` can target it immediately with no further
setup.

**Correction to an earlier statement:** Ghidra was initially reported as absent
because it is installed under `~/src` rather than on `PATH` or in `/Applications`.
It is in fact present and passing all provider checks.

**Resource concern:** 18 GB free is tight for a Rust + GPU-simulation workload,
and 8 GB RAM is below what agentic Ghidra analysis of a large C++ application
comfortably needs. A Rust toolchain is ~1.5 GB; a full CLO3D install (if obtained)
is several GB, and Ghidra's analysis database for a large binary can be multi-GB.
Clearing `~/Library/Caches` (11 GB) and `~/Downloads` (9.1 GB) would roughly
double the available headroom.

---

## 8. Open questions for the user

1. Do you have the CLO3D and/or Marvelous Designer installers/binaries, and where?
2. Is this intended as a **clean-room behavioural reimplementation**, or a
   literal code port? The latter changes the legal risk profile entirely.
3. Which pillars are in scope for v1?
4. Should existing MIT/GarmentCode work be vendored and ported, or reimplemented
   from scratch?
5. Can the repo start as a private/empty scaffold while the RE half is blocked?
