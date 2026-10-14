# Public-manual behavior ledger

Read on 2026-10-14. Entry point: [Marvelous Designer manuals](https://www.marvelousdesigner.com/manuals). The current site exposes section shells; the linked official support articles provide the actual text. No proprietary code, tutorial assets, screenshots or interface artwork were imported into ClothCraft.

| Official article actually read | Behavior identified (paraphrase) | Independent ClothCraft scope | Remaining work |
|---|---|---|---|
| [Polygon](https://support.marvelousdesigner.com/hc/en-us/articles/47358178952345-Polygon) | Build a contour with successive points, close it, cancel points or the operation | Meter-based canvas, 10 mm snap, straight contour validation, either winding, Delaunay compilation | Editable vertices, precise lengths, curves and symmetry |
| [Segment Sewing (2D)](https://support.marvelousdesigner.com/hc/en-us/articles/47358199517209-Segment-Sewing-2D) | Pair boundaries, inspect direction and length difference | Two named full edges, endpoint labels, length delta, explicit reversal/easing and penalty seam | Internal/free sewing and graphical correspondence preview |
| [Move/Rotate Pattern with Gizmo](https://support.marvelousdesigner.com/hc/en-us/articles/47358192192665-Move-Rotate-Pattern-with-Gizmo) | Manipulate a selected panel with directional translation and rotation controls while simulation is off | Independent world-axis arrows/rings, numeric placement, release-to-commit, Escape-to-cancel | Plane handles, local axes and body arrangement guides |

Implementation choices come from our own data and solver contracts: meters in rest coordinates, immutable compiled topology, named edges, whole-document validation, generation invalidation and template history. Similar workflow intent does not imply identical implementation, full feature parity or formal clean-room certification.

## Next reading, not yet specified

- Edit pattern points, curves, internal lines and symmetry.
- Free sewing, 1:N / M:N sewing, notch and seam property tools.
- Arrangement points, bounding volumes and body-aware placement.
- Fabric editing, simulation controls and failure diagnostics.
- Import/export entities, options and fidelity checks, mapped to [our format requirements](../ROADMAP.md#interoperability).

For each new family, record the exact public URL, date, a short paraphrase, an independently written behavior specification, edge cases and status. Do not paste manual pages or treat undocumented internals as known.
