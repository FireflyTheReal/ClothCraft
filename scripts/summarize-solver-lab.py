"""Collect solver_compare reports without treating a faster failure as a win."""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    rows = []
    for path in sorted(args.root.rglob("summary.json")):
        report = json.loads(path.read_text(encoding="utf-8-sig"))
        last = report.get("last", {})
        metrics = last.get("metrics", {})
        rows.append({
            "run": str(path.parent.relative_to(args.root)),
            "backend": report.get("backend", "external-audit"),
            "gpu": report.get("gpu", False),
            "accepted_steps": report.get("accepted_steps"),
            "requested_steps": report.get("requested_steps"),
            "failure": report.get("failure"),
            "median_ms": report.get("median_ms"),
            "p95_ms": report.get("p95_ms"),
            "mean_ms": report.get("mean_ms"),
            "seam_gap_mm": 1000 * metrics.get("max_seam_gap_m", last.get("max_seam_gap_m", 0)),
            "stretch_p95_percent": metrics.get("extension_p95_percent", last.get("extension_p95_percent")),
            "minimum_distance_mm": 1000 * last.get("fresh_minimum_distance_m", last.get("minimum_distance_m", 0)),
            "final_intersection_check": report.get("final_intersection_check", last.get("intersection_check")),
            "first_surface_intersection_step": report.get("first_surface_intersection_step"),
            "panel_speed": last.get("panel_speed"),
        })
    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()
