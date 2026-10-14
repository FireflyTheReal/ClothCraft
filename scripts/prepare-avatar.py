"""Extract the CC0 MakeHuman body, discard helpers, normalize to 1.75 m.
No MakeHuman program code is used. Standard-library only; deterministic export.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "assets/makehuman/base.obj"
vertices, groups = [], {}
group = "default"
for line in SOURCE.read_text(encoding="utf-8").splitlines():
    parts = line.split()
    if not parts:
        continue
    if parts[0] == "v":
        vertices.append(tuple(map(float, parts[1:4])))
    elif parts[0] == "g":
        group = parts[1]
    elif parts[0] == "f":
        face = [int(p.split("/")[0]) - 1 for p in parts[1:]]
        groups.setdefault(group, []).append(face)
faces = groups["body"]
used = sorted({i for face in faces for i in face})
minimum = [min(vertices[i][k] for i in used) for k in range(3)]
maximum = [max(vertices[i][k] for i in used) for k in range(3)]
scale = 1.75 / (maximum[1] - minimum[1])
offset = [(minimum[0] + maximum[0]) / 2, minimum[1], 0]
def transform(p):
    return [(p[k] - offset[k]) * scale for k in range(3)]
mapped = {old: new + 1 for new, old in enumerate(used)}
out = ["# MakeHuman CC0 body; normalized to 1.75 metres; helpers removed", "o mannequin"]
out += ["v " + " ".join(f"{v:.10f}" for v in transform(vertices[i])) for i in used]
triangles = 0
for face in faces:
    for k in range(1, len(face) - 1):
        out.append(f"f {mapped[face[0]]} {mapped[face[k]]} {mapped[face[k+1]]}")
        triangles += 1
(ROOT / "assets/makehuman/mannequin.obj").write_text("\n".join(out) + "\n", encoding="utf-8")
landmarks = {}
for name, faces in groups.items():
    if name.startswith("joint-"):
        ids = {i for face in faces for i in face}
        landmarks[name] = transform([sum(vertices[i][k] for i in ids) / len(ids) for k in range(3)])
report = dict(height_m=1.75, scale=scale, offset_source=offset,
              bounds_m=[transform(minimum), transform(maximum)], vertices=len(used), triangles=triangles,
              source_sha256=hashlib.sha256(SOURCE.read_bytes()).hexdigest(), landmarks=landmarks)
(ROOT / "assets/makehuman/measurements.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps({k: v for k, v in report.items() if k != "landmarks"}, indent=2))
print(json.dumps({k: v for k, v in landmarks.items() if any(s in k for s in ["shoulder", "elbow", "neck", "pelvis", "spine"])}, indent=2))
