"""Summarize isolated law tests; no cloth convergence or performance claim."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("input", type=Path)
args = parser.parse_args()
data = json.loads(args.input.read_text(encoding="utf-8"))
report = {
    "scope": data["scope"],
    "input_sha256": hashlib.sha256(args.input.read_bytes()).hexdigest(),
    "state_time_s": data["state_time_s"],
    "slide_and_cusp": data["slide_and_cusp"],
    "conditioning": {
        "samples": len(data["conditioning"]),
        "failures": [r for r in data["conditioning"] if "error" in r],
        "max_energy_error_j": max(
            (r["energy_error_j"] for r in data["conditioning"] if "error" not in r), default=None),
        "max_probability_error": max(
            (r["max_probability_error"] for r in data["conditioning"] if "error" not in r), default=None),
    },
    "dense_cycle": data["dense_cycle"],
    "dress": [],
}
for radius in sorted({r["radius_m"] for r in data["dress"]}):
    rows = [r for r in data["dress"] if r["radius_m"] == radius]
    accepted = [r for r in rows if "error" not in r["evaluation"]]
    failures = [r for r in rows if "error" in r["evaluation"]]
    probabilities = [
        r["evaluation"][key] for r in accepted
        for key in ["minimum_probability", "maximum_probability"]
        if r["evaluation"][key] is not None
    ]
    report["dress"].append({
        "radius_m": radius,
        "source_edges_sampled": len({r["source_edge"] for r in rows}),
        "evaluations": len(rows),
        "max_target_edges": max(r["nodes"] for r in rows),
        "failed_evaluations": len(failures),
        "failure_reasons": sorted({r["evaluation"]["error"] for r in failures}),
        "max_lower_bound_deficit_j": max(
            (r["evaluation"]["lower_bound_deficit_j"] for r in accepted), default=None),
        "minimum_probability": min(probabilities, default=None),
        "maximum_probability": max(probabilities, default=None),
    })
output = args.input.with_suffix(".summary.json")
output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps({k: v for k, v in report.items() if k not in ["slide_and_cusp", "dense_cycle"]}, indent=2))
