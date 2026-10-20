"""Summarize contact_motion_probe diagnostics; standard library, CPU only.

These are fresh endpoint diagnostics on archived states, not causal attribution
or internal GPU solver traces. No private geometry is needed by this script.
"""
import argparse
import json
import math
from pathlib import Path
from statistics import fmean, median


def norm(v):
    return math.sqrt(sum(x * x for x in v))


def summarize(folder):
    meta = json.loads((folder / "summary.json").read_text(encoding="utf-8"))
    with (folder / "contacts.jsonl").open(encoding="utf-8") as stream:
        rows = [r for line in stream if (r := json.loads(line))["step"] > meta["warmup"]]
    if not rows:
        raise ValueError("No samples after warmup")
    hotspots = []
    for i, description in enumerate(meta["hotspots"]):
        values = [r["hotspots"][i] for r in rows]
        refresh = [r["contact_refresh_force_delta_n"] for r in values if r["contact_refresh_force_delta_n"] is not None]
        hotspots.append({
            **description,
            "active_contact_mean": fmean(r["active_terms"] for r in values),
            "active_key_change_fraction": fmean(r["active_keys_changed"] > 0 for r in values),
            "minimum_ball_radius_m": min(r["ball_radius_m"] for r in values),
            "median_ball_radius_m": median(r["ball_radius_m"] for r in values),
            "mean_contact_force_n": fmean(norm(r["contact_force_n"]) for r in values),
            "mean_sewing_force_n": fmean(norm(r["sewing_force_n"]) for r in values),
            "mean_fresh_balance_residual_n": fmean(r["fresh_balance_residual_n"] for r in values),
            "median_refresh_force_delta_n": median(refresh) if refresh else None,
            "max_refresh_force_delta_n": max(refresh) if refresh else None,
            "nonfinite_refresh_samples": len(values) - len(refresh),
        })
    seams = []
    for s in meta.get("seams", []):
        values = [next(v for v in r["seams"] if v["id"] == s["id"]) for r in rows]
        seams.append({**s,
            "rest_length_ratio": max(s["rest_lengths_m"]) / min(s["rest_lengths_m"]),
            "mean_energy_j": fmean(v["energy_j"] for v in values),
            "peak_gap_m": max(v["max_gap_m"] for v in values),
        })
    return {
        "mode": meta["mode"], "gpu_used": meta["gpu_used"], "avx_used": meta["avx_used"],
        "states_compared_bitwise": meta["states_compared_bitwise"],
        "samples_after_warmup": len(rows),
        "minimum_distance_m": min(r["minimum_distance_m"] for r in rows),
        "mean_detection_reduction_percent": 100 * (1 - meta["simd_detector_ms"]["mean"] / meta["scalar_detector_ms"]["mean"]),
        "timing": {k: meta[k] for k in ("scalar_detector_ms", "simd_detector_ms", "scalar_geometry_ms", "simd_geometry_ms")},
        "hotspots": hotspots, "seams": seams, "limits": meta["limits"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    text = json.dumps(summarize(args.folder), indent=2) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")


if __name__ == "__main__":
    main()
