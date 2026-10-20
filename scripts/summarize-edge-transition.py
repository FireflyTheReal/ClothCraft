"""Summarize the CPU EE transition experiment without treating low motion as convergence."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("root", type=Path)
args = parser.parse_args()


def norm(v):
    return sum(x * x for x in v) ** 0.5


def subtract(a, b):
    return [x - y for x, y in zip(a, b)]


report = {"runs": {}, "interaction": {}, "scope": "CPU VBD, endpoint intersection diagnostics; no GPU timing or long-term equilibrium claim"}
traces = {}
for name in [f"vbd-{mode}-{phase}" for mode in ["baseline", "interior"] for phase in ["quiet", "pull"]]:
    folder = args.root / name
    if not (folder / "summary.json").exists():
        continue
    summary = json.loads((folder / "summary.json").read_text())
    steps = [json.loads(line) for line in (folder / "steps.jsonl").read_text().splitlines()]
    traces[name] = steps
    last = summary["last"]
    report["runs"][name] = {
        "accepted_steps": summary["accepted_steps"], "failure": summary["failure"],
        "final_intersection_check": summary["final_intersection_check"],
        "initial_state_sha256": hashlib.sha256((folder / "initial-state.json").read_bytes()).hexdigest(),
        "mean_step_ms": summary["mean_ms"], "p95_step_ms": summary["p95_ms"],
        "min_endpoint_separation_m": min(s["fresh_minimum_distance_m"] for s in steps),
        "final_sewing_gap_m": last["metrics"]["max_seam_gap_m"],
        "final_extension_p95_percent": last["metrics"]["extension_p95_percent"],
        "final_contact_terms": last["metrics"]["contact_terms"],
        "tail_panel_highpass_rms_m": {p["id"]: (
            sum(next(q["highpass_rms_m"] ** 2 for q in s["panel_speed"] if q["id"] == p["id"]) for s in steps[-30:])
            / len(steps[-30:])) ** 0.5 for p in last["panel_speed"]},
    }
for mode in ["baseline", "interior"]:
    quiet = traces.get(f"vbd-{mode}-quiet")
    pull = traces.get(f"vbd-{mode}-pull")
    if not quiet or not pull:
        continue
    response = [norm(subtract(p["grab_position"], q["grab_position"])) for p, q in zip(pull, quiet)]
    release = [i for i, s in enumerate(pull) if s["phase"] == "release"]
    report["interaction"][mode] = {
        "peak_grab_response_vs_quiet_m": max(response),
        "final_grab_response_vs_quiet_m": response[-1],
        "release_net_motion_m": norm(subtract(pull[-1]["grab_position"], pull[release[0] - 1]["grab_position"])) if release else None,
    }
for name in ["synthetic", "dress-transition"]:
    path = args.root / f"{name}.json"
    if path.exists():
        probe = json.loads(path.read_text())
        report[name] = {"bounds_identical": probe["bounds_identical"],
            "final_sample": probe["samples"][-1], "first_sample": probe["samples"][0]}
(args.root / "comparison.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
