#!/usr/bin/env python3
"""Extract CLO3D's user-facing surface into a machine-readable register.

This reads the *shipped* CLO application and writes a surface inventory: every
recovered command slot, every dialog/dock/view class, the shipped preset
libraries, the Qt resource names, the file extensions the bundle handles and a
symbol-table inventory of all 21 `libClo*` libraries (case-insensitive, which
includes `libCLOAPIInterface`).

The action and screen registers come from the **main executable** only; the
libraries are inventoried for their engine-side namespaces and classes, not for
user-facing commands. Widening the scan does not change the classifier or the
accepted action/screen sets.

What this can and cannot establish:

* **Observed** - symbol names, resource names, preset file names and extension
  counts are read directly from the shipped artifact.
* **Derived** - the grouping of an action into a topic is a heuristic on its
  name. The library namespace/class rankings are counts over `nm` output.
* **Unknown** - the semantics, parameters and UI placement of each action, and
  anything `nm` cannot see (stripped, purely inlined or unnamed code). A name
  such as `OnAddDartFullness` tells you the command exists, not what it does,
  what it costs, or where it lives in a toolbar.

Nothing here executes CLO, modifies it, or copies any of its assets.

Usage:
    scripts/clo3d/extract_surface.py [--app /Applications/CLO.app]
                                     [--out data/clo3d]
                                     [--work clo3d_target]
                                     [--skip-resources]

`--work` holds thinned copies of the target binaries (`libscan/` for the
libraries). Keep it untracked: they are large fourth-party files, and
`/clo3d_target/` is already in `.gitignore`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
from collections import Counter, OrderedDict
from pathlib import Path

# --------------------------------------------------------------------------- #
# Action topics
#
# First matching rule wins. The order is deliberate: a name such as
# `OnAutoSewing` should land in sewing rather than in an "auto" bucket.
# --------------------------------------------------------------------------- #
TOPIC_RULES: list[tuple[str, tuple[str, ...]]] = [
    ("sewing", ("sew", "seam", "stitch", "puckering", "topstitch")),
    ("darts_pleats_folds", ("dart", "pleat", "fullness", "fold", "gather", "shirr")),
    ("notches_marks", ("notch", "annotation", "instruction", "mark")),
    ("grading", ("grad", "size", "spec")),
    ("measurement", ("measure", "Measure", "POM", "ruler", "dimension")),
    ("internal_lines", ("internalline", "offset", "split", "division", "axis")),
    ("pattern_geometry", ("pattern", "point", "curve", "line", "polygon", "piece", "contour", "shape")),
    ("arrangement", ("arrange", "place", "hang", "gizmo", "transform", "move", "rotate", "scale")),
    ("avatar", ("avatar", "Avatar", "mann", "body", "pose", "Pose", "sizing", "skin")),
    ("simulation", ("simulat", "drap", "pin", "Pin", "wind", "Wind", "collision", "pinch", "wrinkle", "layer", "Layer", "physics")),
    ("materials", ("fabric", "Fabric", "material", "Material", "texture", "Texture", "colorway", "colourway", "color", "Color", "palette", "Palette", "PBR", "weave", "knit", "Knit")),
    ("trims", ("zipper", "Zipper", "button", "Button", "trim", "Trim", "accessor", "Accessor", "hardware", "bond", "Bond")),
    ("render", ("render", "Render", "light", "Light", "shadow", "Shadow", "camera", "Camera", "environment", "Environment", "VRAY", "VRay", "denois", "Denois", "turntable", "image", "Image")),
    ("animation", ("anim", "Anim", "motion", "Motion", "record", "Record", "play", "Play", "frame", "Frame", "cache", "Cache", "ABC", "Alembic")),
    ("print", ("print", "Print", "plotter", "Plotter", "PDF")),
    ("interchange", ("export", "Export", "import", "Import", "save", "Save", "load", "Load", "open", "Open", "OBJ", "FBX", "glTF", "GLTF", "USD", "COLLADA", "DXF", "HPGL")),
    ("automation", ("python", "Python", "script", "Script", "API", "plug", "Plug", "AI", "cloud", "Cloud", "connect", "Connect", "marketplace", "CLOFAB")),
    ("editing", ("snap", "grid", "align", "symmetr", "clone", "copy", "mirror", "duplicate", "undo", "redo", "select", "delete", "paste")),
    ("view", ("view", "zoom", "pan", "show", "hide", "display", "activate", "toolbar", "panel", "dock", "window", "layout", "background", "topology")),
    ("project", ("project", "document", "new", "preference", "option", "setting", "language", "manual", "help", "faq", "about", "update", "license")),
]

SCREEN_PREFIXES = ("Dialog", "DockWidget", "View", "Panel", "Widget", "Editor", "Window")


def classify(name: str) -> str:
    """Assign a topic from the command name.

    Needles that are all upper case or contain an underscore are acronyms and
    are matched against the original spelling, because substring matching on the
    lower-cased name confuses them badly: `obj` matches `Object`, `aic` matches
    `AIChatBot`, and `pom` matches `Optimize`.
    """
    lowered = name.lower()
    for topic, needles in TOPIC_RULES:
        for needle in needles:
            if needle.isupper() or "_" in needle:
                if needle in name:
                    return topic
            elif needle in lowered:
                return topic
    return "other"


def strip_on(name: str) -> str:
    return name[2:] if name.startswith("On") and len(name) > 2 and name[2].isupper() else name


def run(cmd: list[str]) -> str:
    return subprocess.run(cmd, capture_output=True, text=True, check=False).stdout


# --------------------------------------------------------------------------- #
# Mach-O symbols
# --------------------------------------------------------------------------- #
def demangle(names: list[str]) -> list[str]:
    if not names:
        return []
    proc = subprocess.run(
        ["c++filt"], input="\n".join(names), capture_output=True, text=True, check=False
    )
    out = proc.stdout.splitlines()
    return out if len(out) == len(names) else names


def thin_arm64(binary: Path, work: Path) -> Path:
    """Ghidra and simple tooling are easier on a thin slice."""
    archs = run(["lipo", "-archs", str(binary)]).split()
    if "arm64" not in archs or len(archs) == 1:
        return binary
    work.mkdir(parents=True, exist_ok=True)
    thin = work / (binary.name + "_arm64")
    if not thin.exists():
        subprocess.run(
            ["lipo", "-thin", "arm64", str(binary), "-output", str(thin)], check=True
        )
    return thin


def bundle_executable(app: Path) -> Path | None:
    """Resolve the real application binary.

    `Contents/MacOS` can hold more than one executable, and picking the first
    one silently selects a helper such as `libvray_usdsupport.so`. Prefer the
    declared `CFBundleExecutable` and fall back to the largest executable.
    """
    plist = app / "Contents/Info.plist"
    declared = run(
        ["/usr/libexec/PlistBuddy", "-c", "Print :CFBundleExecutable", str(plist)]
    ).strip()
    macos = app / "Contents/MacOS"
    candidate = macos / declared
    if declared and candidate.is_file():
        return candidate
    executables = [p for p in macos.iterdir() if p.is_file() and os.access(p, os.X_OK)]
    return max(executables, key=lambda p: p.stat().st_size, default=None)


def read_symbols(binary: Path) -> list[tuple[str, str, str]]:
    """Return (address, type, demangled) for every symbol-table entry."""
    raw = run(["nm", "-a", str(binary)])
    rows: list[tuple[str, str, str]] = []
    mangled: list[str] = []
    for line in raw.splitlines():
        parts = line.split()
        if len(parts) >= 3:
            rows.append((parts[0], parts[1], parts[2]))
            mangled.append(parts[2])
        elif len(parts) == 2:  # undefined: "U _sym"
            rows.append(("", parts[0], parts[1]))
            mangled.append(parts[1])
    demangled = demangle(mangled)
    return [(a, t, d) for (a, t, _), d in zip(rows, demangled)]


# --------------------------------------------------------------------------- #
# Clo library inventory
#
# `nm` reports the symbol table of one architecture. The main executable is
# read in full because the action/screen extraction needs every name; the 21
# `libClo*` libraries are only inventoried, so they are streamed: `nm` output is
# consumed line by line and folded into counters, and only capped per-library
# summaries are retained. That keeps the 194 MB `libCloScene` from ever
# existing in memory as a symbol list.
#
# What `nm` can and cannot see is recorded in the register: it reads the symbol
# table, so stripped or purely inlined code is invisible, local names may be
# absent, and an undefined entry proves an import exists, not where it is used.
# --------------------------------------------------------------------------- #
LIBRARY_GLOB_PREFIX = "libclo"
NM_BATCH = 4096
NM_HEX = re.compile(r"[0-9a-fA-F]+$")

# Ancillary names that begin a mangled symbol and hide its real scope.
SYMBOL_DECORATIONS = (
    "non-virtual thunk to ",
    "virtual thunk to ",
    "construction vtable for ",
    "VTT for ",
    "vtable for ",
    "typeinfo name for ",
    "typeinfo for ",
    "guard variable for ",
    "out-of-line ",
    "reference to ",
    "pointer to ",
)

TEMPLATE_ARGS = re.compile(r"<[^<>]*>")


def clo_libraries(frameworks: Path) -> list[Path]:
    """Every `libClo*.dylib` in the frameworks directory, case-insensitively.

    The bundle ships `libClo...` and one `libCLOAPIInterface`, so a
    case-sensitive glob would silently drop the API library.
    """
    if not frameworks.is_dir():
        return []
    return sorted(
        path
        for path in frameworks.iterdir()
        if path.is_file()
        and path.suffix == ".dylib"
        and path.name.lower().startswith(LIBRARY_GLOB_PREFIX)
    )


def parse_nm_line(line: str) -> tuple[str, str] | None:
    """Split one `nm` line into (type, mangled name).

    `nm` prints `<address> <type> <name>` for defined entries and a blank
    address for undefined ones, so a two-field line is `U <name>`.
    """
    if not line.strip():
        return None
    parts = line.rstrip("\n").split(None, 2)
    if len(parts) == 2:
        return parts[0], parts[1]
    if len(parts) >= 3:
        if NM_HEX.fullmatch(parts[0]):
            return parts[1], parts[2]
        return parts[0], parts[2]
    return None


def strip_decorations(name: str) -> str:
    """Remove vtable/thunk/typeinfo prefixes to expose the real scope."""
    changed = True
    while changed:
        changed = False
        for prefix in SYMBOL_DECORATIONS:
            if name.startswith(prefix):
                name = name[len(prefix) :]
                changed = True
    return name


def owner_scope(name: str) -> tuple[str | None, str | None]:
    """Return (top_level_namespace, owner_scope) for a demangled name.

    `Marvelous::Fabric::FabricItem::SetColor(FabricColor)` yields
    `("Marvelous", "FabricItem")`. Data symbols without a parameter list treat
    their last component as the owner (`vtable for Marvelous::Scene` yields
    `("Marvelous", "Scene")`). Names with no qualified scope yield `(None, None)`.
    """
    text = strip_decorations(name)
    had_parameters = "(" in text
    if had_parameters:
        text = text[: text.find("(")]
    while True:
        stripped = TEMPLATE_ARGS.sub("", text)
        if stripped == text:
            break
        text = stripped
    if "::" not in text:
        return None, None
    components = [part.strip() for part in text.split("::") if part.strip()]
    if len(components) < 2:
        return None, None
    top = components[0]
    owner = components[-2] if had_parameters else components[-1]
    return top or None, owner or None


def top_entries(counter: Counter, limit: int) -> list[dict]:
    """Deterministic `[{name, count}]`, ties broken by name."""
    ranked = sorted(counter.items(), key=lambda kv: (-kv[1], kv[0]))
    return [{"name": name, "count": count} for name, count in ranked[:limit]]


def scan_library(library: Path, work: Path) -> tuple[dict, Counter, Counter]:
    """Inventory one library by streaming `nm -a` output.

    Returns the per-library record plus the namespace and owner-scope counters
    so the caller can merge them across the bundle. Only counters are kept: no
    per-symbol list is retained for a library.
    """
    thin = thin_arm64(library, work)
    by_type: Counter[str] = Counter()
    namespaces: Counter[str] = Counter()
    classes: Counter[str] = Counter()
    symbol_entries = 0
    defined = 0
    undefined = 0
    debug_entries = 0
    demangled_names = 0
    pending: list[str] = []

    def flush(names: list[str]) -> None:
        nonlocal demangled_names
        if not names:
            return
        for mangled, text in zip(names, demangle(names)):
            if text != mangled:
                demangled_names += 1
            top, owner = owner_scope(text)
            if top:
                namespaces[top] += 1
            if owner:
                classes[owner] += 1

    proc = subprocess.Popen(
        ["nm", "-a", str(thin)],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        bufsize=1 << 20,
    )
    assert proc.stdout is not None
    for line in proc.stdout:
        parsed = parse_nm_line(line)
        if parsed is None:
            continue
        symbol_type, mangled = parsed
        symbol_entries += 1
        by_type[symbol_type] += 1
        if symbol_type == "U":
            undefined += 1
            continue
        if symbol_type == "-":
            # Debug/stab entry: not a code symbol, and its payload is a field
            # dump rather than a name, so it must not reach the rankings.
            debug_entries += 1
            continue
        defined += 1
        pending.append(mangled)
        if len(pending) >= NM_BATCH:
            flush(pending)
            pending.clear()
    flush(pending)
    nm_exit = proc.wait()

    record = {
        "name": library.name,
        "bytes": library.stat().st_size,
        "architectures": run(["lipo", "-archs", str(library)]).split(),
        "analysed_slice": str(thin),
        "slice_bytes": thin.stat().st_size,
        "slice_sha256": sha256(thin),
        "symbol_entries": symbol_entries,
        "defined": defined,
        "undefined": undefined,
        "debug_entries": debug_entries,
        "demangled_names": demangled_names,
        "distinct_top_namespaces": len(namespaces),
        "distinct_engine_classes": len(classes),
        "by_symbol_type": dict(sorted(by_type.items())),
        "top_namespaces": top_entries(namespaces, 12),
        "top_engine_classes": top_entries(classes, 12),
    }
    if nm_exit != 0:
        record["nm_exit_code"] = nm_exit
    return record, namespaces, classes


def scan_libraries(frameworks: Path, work: Path) -> dict:
    """Inventory every Clo library and merge the bundle-wide totals."""
    libraries = clo_libraries(frameworks)
    library_work = work / "libscan"
    library_work.mkdir(parents=True, exist_ok=True)

    records: list[dict] = []
    bundle_namespaces: Counter[str] = Counter()
    bundle_classes: Counter[str] = Counter()
    total_symbols = 0
    total_defined = 0
    total_undefined = 0
    total_debug = 0

    for library in libraries:
        print(f"==> Scanning {library.name}", flush=True)
        record, namespaces, classes = scan_library(library, library_work)
        print(
            f"    {record['symbol_entries']} entries "
            f"({record['defined']} defined, {record['undefined']} undefined)"
        )
        records.append(record)
        bundle_namespaces.update(namespaces)
        bundle_classes.update(classes)
        total_symbols += record["symbol_entries"]
        total_defined += record["defined"]
        total_undefined += record["undefined"]
        total_debug += record["debug_entries"]

    largest = max(records, key=lambda r: r["symbol_entries"], default=None)
    return {
        "scan": {
            "frameworks": str(frameworks),
            "pattern": "libClo*.dylib (case-insensitive)",
            "libraries": len(records),
            "analysed_arch": "arm64",
            "demangler": "c++filt",
            "symbol_entries": total_symbols,
            "defined": total_defined,
            "undefined": total_undefined,
            "debug_entries": total_debug,
            "largest_library": (
                {
                    "name": largest["name"],
                    "bytes": largest["bytes"],
                    "symbol_entries": largest["symbol_entries"],
                }
                if largest
                else None
            ),
            "note": (
                "nm reads the symbol table of the arm64 slice. Stripped, purely "
                "inlined and unnamed local code is invisible; an undefined entry "
                "proves an import, not its call sites. `defined + undefined + "
                "debug_entries == symbol_entries`; debug entries are stab records, "
                "not code symbols, and are excluded from the rankings."
            ),
        },
        "libraries": records,
        "top_engine_classes": top_entries(bundle_classes, 20),
        "top_namespaces": top_entries(bundle_namespaces, 20),
    }


# --------------------------------------------------------------------------- #
# Qt resource collection (.rcc)
# --------------------------------------------------------------------------- #
def parse_rcc(path: Path) -> dict:
    """Read the name table of a Qt resource file.

    The header is `qres`, version, tree offset, data offset, name offset. Each
    name-table entry observed in this build is

        quint16 length, quint32 hash, length x UTF-16BE

    which is why the obvious `quint16 length` + characters reading produces
    mojibake after the first name.

    Directory nesting is **not** reconstructed: the tree section stores a child
    offset encoding that this scan did not decode, so `names` is the name table
    in file order with directories and files interleaved. Use it as an inventory
    of what exists, not as a path listing.
    """
    data = path.read_bytes()
    if data[:4] != b"qres":
        return {"error": "not a Qt resource file"}
    version, tree_off, data_off, name_off = struct.unpack(">IIII", data[4:20])

    names: list[str] = []
    offset = name_off
    limit = len(data)
    while offset + 6 <= limit:
        length = struct.unpack(">H", data[offset : offset + 2])[0]
        if length == 0 or length > 512:
            break
        end = offset + 6 + length * 2
        if end > limit:
            break
        try:
            names.append(data[offset + 6 : end].decode("utf-16-be"))
        except UnicodeDecodeError:
            break
        offset = end

    files = [n for n in names if "." in n]
    return {
        "version": version,
        "declared_tree_offset": tree_off,
        "declared_data_offset": data_off,
        "declared_name_offset": name_off,
        "name_count": len(names),
        "names": names,
        "by_extension": dict(
            Counter(n.rsplit(".", 1)[-1].lower() for n in files).most_common()
        ),
        "note": "Name table in file order; directory nesting not reconstructed.",
    }


# --------------------------------------------------------------------------- #
# Scans
# --------------------------------------------------------------------------- #
def scan_presets(root: Path) -> dict:
    categories: "OrderedDict[str, dict]" = OrderedDict()
    if not root.is_dir():
        return {"categories": categories, "error": "no Preset directory"}
    for directory in sorted(p for p in root.iterdir() if p.is_dir()):
        files = [f for f in directory.rglob("*") if f.is_file()]
        ext = Counter(f.suffix.lower().lstrip(".") or "(none)" for f in files)
        samples = sorted(str(f.relative_to(directory)) for f in files)[:8]
        categories[directory.name] = {
            "file_count": len(files),
            "by_extension": dict(ext.most_common()),
            "sample": samples,
        }
    return {"categories": categories}


def scan_extensions(contents: Path) -> dict:
    """Extension histogram for the whole bundle except the vendor frameworks.

    `Contents/Frameworks` is excluded because those are third-party dependencies
    rather than CLO content, and they would swamp the histogram. Scanning all of
    `Contents` rather than only `Resources` matters: CLO keeps its own asset
    containers (`Assets/`) outside the resources tree.
    """
    counter: Counter[str] = Counter()
    for path in contents.rglob("*"):
        if not path.is_file():
            continue
        if "Frameworks" in path.parts:
            continue
        counter[path.suffix.lower().lstrip(".") or "(none)"] += 1
    return dict(counter.most_common())


def tool_icon_register(names: list[str]) -> list[str]:
    """Resource names that look like tool / toolbar artwork.

    Heuristic: SVG or PNG payloads whose name mentions an icon, a tool or a
    toolbar. Reported as candidate controls, not as a toolbar layout.
    """
    needles = ("icon", "tool", "button", "cursor")
    out = []
    for name in names:
        lowered = name.lower()
        if not lowered.endswith((".svg", ".png")):
            continue
        if any(n in lowered for n in needles):
            out.append(name)
    return sorted(set(out))


# --------------------------------------------------------------------------- #
# Main
# --------------------------------------------------------------------------- #
def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", default="/Applications/CLO.app")
    parser.add_argument("--out", default="data/clo3d")
    parser.add_argument("--work", default="clo3d_target")
    parser.add_argument("--skip-resources", action="store_true")
    args = parser.parse_args()

    app = Path(args.app)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    if not app.is_dir():
        print(f"CLO application not found at {app}", file=sys.stderr)
        return 1

    exe = bundle_executable(app)
    if exe is None:
        print("No executable found in Contents/MacOS", file=sys.stderr)
        return 1

    print(f"==> Main executable: {exe.name} ({exe.stat().st_size} bytes)")
    thin = thin_arm64(exe, Path(args.work))
    if thin != exe:
        print(f"    analysing thinned slice {thin}")

    print("==> Reading symbols")
    symbols = read_symbols(thin)
    print(f"    {len(symbols)} symbol-table entries")

    # --- actions ----------------------------------------------------------- #
    slot_classes = {
        "View2DActionSlots": "2d",
        "View3DActionSlots": "3d",
        "MainMenuActionSlots": "menu",
    }
    actions: list[dict] = []
    seen_actions: set[tuple[str, str]] = set()
    for _, _, demangled in symbols:
        for cls, view in slot_classes.items():
            marker = f"{cls}::"
            idx = demangled.find(marker)
            if idx < 0:
                continue
            match = re.match(r"([A-Za-z_][A-Za-z0-9_]*)", demangled[idx + len(marker) :])
            if not match:
                continue
            method = match.group(1)
            if (cls, method) in seen_actions:
                continue
            seen_actions.add((cls, method))
            action = strip_on(method)
            actions.append(
                {
                    "view": view,
                    "class": cls,
                    "symbol": method,
                    "action": action,
                    "topic": classify(action),
                }
            )
    actions.sort(key=lambda a: (a["view"], a["topic"], a["action"]))

    # --- screens ----------------------------------------------------------- #
    screens: dict[str, dict] = {}
    pattern = re.compile(r"\b((?:" + "|".join(SCREEN_PREFIXES) + r")[A-Za-z0-9_]*)::")
    for _, _, demangled in symbols:
        for match in pattern.finditer(demangled):
            cls = match.group(1)
            screens.setdefault(cls, {"class": cls, "methods": 0})
            screens[cls]["methods"] += 1
    for entry in screens.values():
        entry["kind"] = next(
            (p for p in SCREEN_PREFIXES if entry["class"].startswith(p)), "other"
        )
    screen_list = sorted(screens.values(), key=lambda s: (-s["methods"], s["class"]))

    # --- namespaces -------------------------------------------------------- #
    namespaces = Counter()
    for _, _, demangled in symbols:
        for match in re.finditer(r"\b([A-Z][A-Za-z0-9_]*)::", demangled):
            namespaces[match.group(1)] += 1

    resources_root = app / "Contents/Resources"
    presets = scan_presets(resources_root / "Preset")
    extensions = scan_extensions(app / "Contents")

    rcc: dict = {}
    icons: list[str] = []
    if not args.skip_resources:
        rcc_path = resources_root / "Resources_Light.rcc"
        if rcc_path.is_file():
            print("==> Parsing Resources_Light.rcc")
            rcc = parse_rcc(rcc_path)
            if "error" in rcc:
                print(f"    {rcc['error']}", file=sys.stderr)
            else:
                icons = tool_icon_register(rcc["names"])
                print(
                    f"    {rcc['name_count']} resource names, "
                    f"{len(icons)} candidate tool icons"
                )

    topic_counts = Counter(a["topic"] for a in actions)
    view_counts = Counter(a["view"] for a in actions)

    libraries = scan_libraries(app / "Contents/Frameworks", Path(args.work))
    library_scan = libraries["scan"]
    bundle_symbols = len(symbols) + library_scan["symbol_entries"]

    surface = {
        "target": {
            "app": str(app),
            "version": read_version(app),
            "main_binary": exe.name,
            "main_binary_bytes": exe.stat().st_size,
            "analysed_slice": str(thin),
            "sha256": sha256(thin),
            "symbol_entries": len(symbols),
        },
        "action_counts": {
            "total": len(actions),
            "by_view": dict(view_counts),
            "by_topic": dict(topic_counts),
        },
        "screen_counts": {
            "total": len(screen_list),
            "by_kind": dict(Counter(s["kind"] for s in screen_list)),
        },
        "resource_counts": {
            "names": rcc.get("name_count", 0),
            "candidate_tool_icons": len(icons),
            "by_extension": rcc.get("by_extension", {}),
        },
        "library_counts": {
            "libraries_scanned": library_scan["libraries"],
            "library_symbol_entries": library_scan["symbol_entries"],
            "library_defined": library_scan["defined"],
            "library_undefined": library_scan["undefined"],
            "library_debug_entries": library_scan["debug_entries"],
            "bundle_symbol_entries": bundle_symbols,
            "largest_library": library_scan["largest_library"],
            "top_engine_classes": libraries["top_engine_classes"],
        },
        "extensions_in_bundle": extensions,
    }

    write_json(out / "surface.json", surface)
    write_json(out / "actions.json", {"actions": actions})
    write_json(out / "screens.json", {"screens": screen_list})
    write_json(out / "namespaces.json", {"namespaces": dict(namespaces.most_common(120))})
    write_json(out / "libraries.json", libraries)
    write_json(out / "presets.json", presets)
    write_json(out / "resources.json", rcc)
    write_json(out / "tool_icons.json", {"icons": icons})

    write_actions_markdown(out / "ACTIONS.md", actions, surface)
    write_readme(out / "README.md", surface)

    print(
        f"==> Wrote {out}/ ({len(actions)} actions, {len(screen_list)} screen classes, "
        f"{library_scan['libraries']} libraries, {bundle_symbols} bundle symbol entries)"
    )
    return 0


def read_version(app: Path) -> str:
    plist = app / "Contents/Info.plist"
    return run(
        ["/usr/libexec/PlistBuddy", "-c", "Print :CFBundleShortVersionString", str(plist)]
    ).strip()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, payload: object) -> None:
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_actions_markdown(path: Path, actions: list[dict], surface: dict) -> None:
    lines = [
        "# CLO3D action register (generated)",
        "",
        f"Target: CLO `{surface['target']['version']}` - main binary `{surface['target']['main_binary']}`.",
        f"{len(actions)} distinct command slots recovered from three action-slot classes.",
        "",
        "Names are observed from the shipped symbol table. Topics are a heuristic on the",
        "name and are a navigation aid only; they are not a claim about behaviour.",
        "",
    ]
    titles = {"2d": "2D pattern window", "3d": "3D garment window", "menu": "Main menu"}
    for view in ("2d", "3d", "menu"):
        subset = [a for a in actions if a["view"] == view]
        lines += [f"## {titles[view]} ({len(subset)})", ""]
        by_topic: dict[str, list[dict]] = {}
        for a in subset:
            by_topic.setdefault(a["topic"], []).append(a)
        for topic in sorted(by_topic):
            lines += [f"### {topic} ({len(by_topic[topic])})", ""]
            lines += [f"- `{a['symbol']}`" for a in by_topic[topic]]
            lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def write_readme(path: Path, surface: dict) -> None:
    t = surface["target"]
    lib = surface.get("library_counts", {})
    lines = [
        "# CLO3D surface database",
        "",
        "Generated by `scripts/clo3d/extract_surface.py`. Re-run it to refresh.",
        "",
        "| File | Contents |",
        "|---|---|",
        "| `surface.json` | Target identity, totals, extension histogram |",
        "| `actions.json` | Every recovered command slot, with view and topic |",
        "| `screens.json` | Every dialog / dock / view / panel class, by method count |",
        "| `namespaces.json` | Most frequent top-level namespaces |",
        "| `libraries.json` | Per-library symbol inventory of all 21 `libClo*` libraries |",
        "| `presets.json` | Shipped preset libraries and their file types |",
        "| `resources.json` | The Qt resource name table |",
        "| `tool_icons.json` | Resource names that look like tool / toolbar artwork |",
        "| `ACTIONS.md` | The same action register, grouped for reading |",
        "",
        "## Target",
        "",
        f"- CLO version `{t['version']}`",
        f"- main binary `{t['main_binary']}` ({t['main_binary_bytes']} bytes)",
        f"- analysed slice `{t['analysed_slice']}`",
        f"- slice SHA-256 `{t['sha256']}`",
        f"- symbol-table entries `{t['symbol_entries']}`",
        "",
        "## Library coverage",
        "",
        "The action and screen registers come from the **main executable** only. The",
        "21 `libClo*` libraries in `Contents/Frameworks` are inventoried separately",
        "(case-insensitive match, so `libCLOAPIInterface` is included): each is thinned",
        "to `arm64`, read with `nm -a` and demangled with `c++filt`.",
        "",
        f"- libraries scanned `{lib.get('libraries_scanned', 0)}`",
        f"- library symbol entries `{lib.get('library_symbol_entries', 0)}` "
        f"({lib.get('library_defined', 0)} defined, {lib.get('library_undefined', 0)} undefined, "
        f"{lib.get('library_debug_entries', 0)} debug)",
        f"- whole-bundle symbol entries (main + libraries) `{lib.get('bundle_symbol_entries', 0)}`",
        "",
        "`libraries.json` carries the per-library records, the bundle-wide namespace",
        "ranking and the top engine classes. Only counters are retained per library;",
        "`nm` output is streamed, so `libCloScene` (194 MB) is never held in memory as",
        "a symbol list.",
        "",
        "## What this is not",
        "",
        "This is a **surface** inventory. It establishes that a named command exists and",
        "which view owns it. It does not establish behaviour, parameters, default values,",
        "cost, or where the control sits in a toolbar. Treat every entry as a question to",
        "ask of the decompiler, not as documentation.",
        "",
        "Named gaps: the resource name table is reported in file order with directory",
        "nesting not reconstructed, topic assignment is a name heuristic, and `nm` only",
        "sees the symbol table, so stripped, purely inlined or unnamed code is invisible.",
        "",
    ]
    path.write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
