"""Summarize motion_probe traces using Python's standard library only.

This measures accepted simulation positions, not displayed frames. The second
difference removes constant-velocity drift; physical vibration can also appear
in this measure. Keep the initial scene, dt and vertex selection identical.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
from statistics import fmean


def percentile(values, p):
    values = sorted(values)
    index = (len(values) - 1) * p
    lo = int(index)
    hi = min(lo + 1, len(values) - 1)
    return values[lo] + (index - lo) * (values[hi] - values[lo])


def summarize(folder, min_y, warmup):
    with (folder / "trajectory.jsonl").open(encoding="utf-8") as stream:
        first_line = next(stream)
        first = json.loads(first_line)
        states = [first, *(json.loads(line) for line in stream)]
    ids = [i for i, p in enumerate(first["x"]) if p[1] >= min_y]
    if not ids or len(states) <= warmup + 2:
        raise ValueError("Selection or measured interval is empty")
    highpass = []
    vertical_steps = []
    for a, b, c in zip(states[warmup:], states[warmup + 1:], states[warmup + 2:]):
        for i in ids:
            delta = [b["x"][i][k] - (a["x"][i][k] + c["x"][i][k]) * 0.5 for k in range(3)]
            highpass.append(math.sqrt(sum(d * d for d in delta)) * 1000)
            vertical_steps.append(abs(b["x"][i][1] - a["x"][i][1]) * 1e6)
    metrics = [json.loads(line) for line in (folder / "metrics.jsonl").read_text().splitlines()]
    measured = metrics[warmup:]
    last = metrics[-1]
    return {
        "directory": str(folder),
        "initial_state_sha256": hashlib.sha256(first_line.encode()).hexdigest(),
        "vertices": len(first["x"]), "selected_vertices": len(ids),
        "selection": f"initial world y >= {min_y} m", "warmup_steps": warmup,
        "start_time_s": first["time"], "end_time_s": states[-1]["time"],
        "steps": len(metrics), "iterations": last["iterations"], "dt_s": last["dt_s"],
        "highpass_position_mm": {
            "definition": "norm(x[t] - (x[t-1]+x[t+1])/2); no rendering filter",
            "rms": math.sqrt(fmean(v * v for v in highpass)),
            "p95": percentile(highpass, .95), "max": max(highpass),
        },
        "max_vertical_step_um": max(vertical_steps),
        "mean_full_step_ms_after_warmup": fmean(m["step_ms"] for m in measured),
        "mean_detection_ms_after_warmup": fmean(m["detection_ms"] for m in measured),
        "mean_solve_ms_after_warmup": fmean(m["solve_ms"] for m in measured),
        "final_max_seam_gap_mm": last["max_seam_gap_m"] * 1000,
        "final_extension_p95_percent": last["extension_p95_percent"],
        "minimum_detected_separation_mm": min(m["minimum_distance_m"] for m in metrics) * 1000,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folders", type=Path, nargs="+")
    parser.add_argument("--min-y", type=float, default=1.35)
    parser.add_argument("--warmup", type=int, default=60)
    args = parser.parse_args()
    if args.warmup < 0:
        parser.error("warmup must be nonnegative")
    print(json.dumps([summarize(p, args.min_y, args.warmup) for p in args.folders], indent=2))
