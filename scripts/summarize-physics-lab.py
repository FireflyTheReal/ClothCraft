"""Summarize sequential headless runs with identical input and mouse controls."""
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def rows(path):
    with path.open(encoding="utf-8-sig") as stream:
        for line in stream:
            yield json.loads(line)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def distribution(values):
    values = sorted(values)
    return {"mean": statistics.mean(values), "p50": statistics.median(values),
            "p95": values[math.ceil(0.95 * len(values)) - 1], "max": values[-1]}


def summarize(root):
    result = {"scope": "Full solver step wall time excludes offline audits and trace export; oracle timings are validation only.", "runs": {}}
    controls = list(rows(root / "isotropic/controls.jsonl"))
    initial_reference = None
    for name in ("isotropic", "directional", "oracle"):
        run = root / name
        summary = read(run / "summary.json")
        if controls != list(rows(run / "controls.jsonl")):
            raise ValueError(f"Different mouse controls: {name}")
        energy = list(rows(run / "baseline/energy.jsonl"))
        metrics = list(rows(run / "baseline/metrics.jsonl"))
        trajectory = rows(run / "baseline/trajectory.jsonl")
        initial = next(trajectory)
        if initial_reference is None:
            initial_reference = initial
        if initial != initial_reference:
            raise ValueError(f"Different initial accepted state: {name}")
        final = initial
        settled = None
        count = 0
        for final in trajectory:
            count += 1
            if count == 120:
                settled = final
        if not (count == len(metrics) == len(energy) == len(controls)):
            raise ValueError(f"Incomplete replay: {name}")
        ids = read(run / "annotations.json")["initial_isotropic_restricted_vertices"]
        movement = [1000 * math.dist(initial["x"][i], final["x"][i]) for i in ids]
        out = {"steps": count, "simulated_seconds": final["time"] - initial["time"],
               "step_ms": distribution([m["step_ms"] for m in metrics]),
               "fresh_minimum_distance_m": min(e["fresh_minimum_distance_m"] for e in energy),
               "final_minimum_area_ratio": energy[-1]["minimum_area_ratio"],
               "final_collapsed_faces": energy[-1]["faces_below_one_percent_rest_area"],
               "final_seam_gap_m": metrics[-1]["max_seam_gap_m"],
               "final_extension_p95_percent": metrics[-1]["extension_p95_percent"],
               "initially_restricted_vertices": len(ids),
               "restricted_net_motion_mm": distribution(movement) if movement else None,
               "restricted_settle_motion_mm": distribution([
                   1000 * math.dist(initial["x"][i], settled["x"][i]) for i in ids
               ]) if ids and settled else None,
               "directional_vertices": distribution([m["directional_vertices"] for m in metrics]),
               "precision_ms": distribution([m["precision_ms"] for m in metrics]),
               "final_highpass_rms_m": energy[-1]["highpass_rms_m"],
               "final_intersection_check": summary["fresh_final_intersection_check"],
               "phase_step_ms": {}}
        for phase in ("settle", "pull", "hold", "release"):
            indices = [i for i, e in enumerate(energy) if e["phase"] == phase]
            if indices:
                out["phase_step_ms"][phase] = distribution([metrics[i]["step_ms"] for i in indices])
                start = max(0, indices[0] - 1)
                out[f"{phase}_attachment_motion_mm"] = 1000 * math.dist(energy[start]["attachment_m"], energy[indices[-1]]["attachment_m"])
        result["runs"][name] = out
    result["oracle_trajectory_identical"] = digest(root / "directional/baseline/trajectory.jsonl") == digest(root / "oracle/baseline/trajectory.jsonl")
    if not result["oracle_trajectory_identical"]:
        raise ValueError("The CPU contact oracle changed the accepted trajectory")
    return result


if __name__ == "__main__":
    root = Path(sys.argv[1])
    result = summarize(root)
    (root / "comparison.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
