"""Compare equal-duration continuations of one local issue capture (stdlib only)."""
import argparse
import json
import math
import statistics
from pathlib import Path


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def summarize(capture, runs):
    report = read(capture / "report.json")
    original = read(capture / report["project"])["state"]
    positions = {tuple(p): i for i, p in enumerate(original["x"])}
    # Resolve the capture's exact accepted coordinates, not persisted mesh IDs.
    marked, restricted = set(), set()
    for name in ("marked_faces", "most_restricted_faces"):
        for face in report["analysis"].get(name, []):
            bounds = face.get("fresh_ogc_bound_m") or [math.inf] * 3
            for p, bound in zip(face["positions_m"], bounds):
                i = positions[tuple(p)]
                if name == "marked_faces":
                    marked.add(i)
                if bound < 1e-5:
                    restricted.add(i)
    result = {}
    durations = []
    for directory in runs:
        state = read(directory / "fitting-project.json")["state"]
        rows = [json.loads(line) for line in (directory / "metrics.jsonl").read_text().splitlines()]
        if not rows or len(state["x"]) != len(original["x"]):
            raise ValueError("A nonempty run with unchanged topology is required")
        durations.append(state["time"] - original["time"])
        values = sorted(row["step_ms"] for row in rows)
        out = {
            "steps": len(rows), "simulated_s": durations[-1],
            "step_mean_ms": statistics.mean(values),
            "step_median_ms": statistics.median(values),
            "step_p95_ms": values[math.ceil(0.95 * len(values)) - 1],
            "minimum_observed_detection_distance_m": min(r["minimum_distance_m"] for r in rows),
            "final_seam_gap_mm": 1000 * rows[-1]["max_seam_gap_m"],
            "final_extension_p95_percent": rows[-1]["extension_p95_percent"],
            "final_extension_max_percent": rows[-1]["extension_max_percent"],
            "precision_mean_ms": statistics.mean(r.get("precision_ms", 0) for r in rows),
            "steps_with_precision_work": sum(r.get("precision_vertices", 0) > 0 for r in rows),
        }
        for label, ids in [("marked", marked), ("initially_restricted", restricted)]:
            motion = [1000 * math.dist(original["x"][i], state["x"][i]) for i in ids]
            out[label] = {"vertices": len(ids), "motion_mm_min_median_max": [min(motion), statistics.median(motion), max(motion)] if motion else None}
        result[str(directory)] = out
    if max(durations) - min(durations) > 1e-8:
        raise ValueError("Compare runs with the same simulated duration")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("runs", type=Path, nargs="+")
    args = parser.parse_args()
    print(json.dumps(summarize(args.capture, args.runs), indent=2))
