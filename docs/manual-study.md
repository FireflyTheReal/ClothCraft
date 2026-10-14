# Public-manual behavior ledger

Read on 2026-10-14. Entry point: [Marvelous Designer manuals](https://www.marvelousdesigner.com/manuals). The current site exposes section shells; the linked official support articles provide the actual text. No proprietary code, tutorial assets, screenshots or interface artwork were imported into ClothCraft.

| Official article actually read | Behavior identified (paraphrase) | Independent ClothCraft scope | Remaining work |
|---|---|---|---|
| [Polygon](https://support.marvelousdesigner.com/hc/en-us/articles/47358178952345-Polygon) | Build a contour with successive points, close it, cancel points or the operation | Meter-based canvas, 10 mm snap, straight contour validation, either winding, Delaunay compilation | Precise lengths, holes and symmetry (editable points/curves implemented in the follow-up below) |
| [Segment Sewing (2D)](https://support.marvelousdesigner.com/hc/en-us/articles/47358199517209-Segment-Sewing-2D) | Pair boundaries, inspect direction and length difference | Two named full edges, endpoint labels, length delta, explicit reversal/easing and penalty seam | Internal/free sewing; basic correspondence lines are implemented in the follow-up below |
| [Move/Rotate Pattern with Gizmo](https://support.marvelousdesigner.com/hc/en-us/articles/47358192192665-Move-Rotate-Pattern-with-Gizmo) | Manipulate a selected panel with directional translation and rotation controls while simulation is off | Independent world-axis arrows/rings, numeric placement, release-to-commit, Escape-to-cancel | Plane handles, local axes and body arrangement guides |

Implementation choices come from our own data and solver contracts: meters in rest coordinates, immutable compiled topology, named edges, whole-document validation, generation invalidation and template history. Similar workflow intent does not imply identical implementation, full feature parity or formal clean-room certification.

## Next reading, not yet specified

- Internal lines, holes, precise dimensions and symmetry; basic curve/point editing is implemented.
- Free sewing, 1:N / M:N sewing, notch and seam property tools.
- Arrangement points, bounding volumes and body-aware placement.
- Fabric editing, simulation controls and failure diagnostics.
- Import/export entities, options and fidelity checks, mapped to [our format requirements](../ROADMAP.md#interoperability).

For each new family, record the exact public URL, date, a short paraphrase, an independently written behavior specification, edge cases and status. Do not paste manual pages or treat undocumented internals as known.

## Unified-board follow-up (2026-10-14)

Read [Edit AI Bezier Curve](https://support.marvelousdesigner.com/hc/en-us/articles/47358445584537-Edit-AI-Bezier-Curve) and [3D Segment Sewing](https://support.marvelousdesigner.com/hc/en-us/articles/47358302592409-3D-Segment-Sewing-VER4-2-0). The observed behaviors are editable curve handles, conversion between straight/curved boundaries, and pairing garment segments directly in 3D with shared seam visibility. ClothCraft now provides independent implementations of those basic interactions; modifier conventions and complete feature parity are not claimed.

The [W3C SVG path specification](https://www.w3.org/TR/SVG/paths.html) defines the cubic representation (two endpoints and two controls). ClothCraft uses that standard representation with its own de Casteljau subdivision, a fixed 0.25 mm tolerance, and stable edge-name reconstruction after meshing. Source curves and sewing identities are preserved; exact curve intersection predicates remain outside this version.
