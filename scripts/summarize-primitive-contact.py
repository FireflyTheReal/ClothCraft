"""Summarize finite-primitive ablations; keep geometric and force-law gates separate."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("root", type=Path)
parser.add_argument("--plot", type=Path)
args = parser.parse_args()


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def distance(a, b):
    return sum((x - y) ** 2 for x, y in zip(a, b)) ** 0.5


report = {"scope": "CPU only; 40 steps = 2/3 s, not an equilibrium certificate",
          "runs": {}, "interaction": {}, "transitions": {}, "slide": {}}
traces = {}
hashes = set()
for mode in ["control", "primitive"]:
    for phase in ["quiet", "pull"]:
        name = f"{mode}-{phase}"
        folder = args.root / name
        summary = read(folder / "summary.json")
        protocol = read(folder / "protocol.json")
        steps = [json.loads(line) for line in (folder / "steps.jsonl").read_text(encoding="utf-8").splitlines()]
        traces[name] = steps
        state_hash = hashlib.sha256((folder / "initial-state.json").read_bytes()).hexdigest()
        hashes.add(state_hash)
        last = summary["last"]
        report["runs"][name] = {
            "accepted_steps": summary["accepted_steps"], "failure": summary["failure"],
            "final_intersection_check": summary["final_intersection_check"],
            "initial_state_sha256": state_hash, "iterations": protocol["settings"]["iterations"],
            "dt": protocol["settings"]["dt"], "mean_full_step_ms": summary["mean_ms"],
            "p95_full_step_ms": summary["p95_ms"],
            "minimum_endpoint_distance_m": min(s["fresh_minimum_distance_m"] for s in steps),
            "final_sewing_gap_m": last["metrics"]["max_seam_gap_m"],
            "final_extension_p95_percent": last["metrics"]["extension_p95_percent"],
            "final_contact_terms": last["metrics"]["contact_terms"],
        }
    quiet, pull = traces[f"{mode}-quiet"], traces[f"{mode}-pull"]
    release = next(i for i, step in enumerate(pull) if step["phase"] == "release")
    report["interaction"][mode] = {
        "peak_response_vs_quiet_m": max(distance(p["grab_position"], q["grab_position"]) for p, q in zip(pull, quiet)),
        "release_net_motion_m": distance(pull[-1]["grab_position"], pull[release-1]["grab_position"]),
        "release_steps": len(pull)-release,
    }
assert len(hashes) == 1, "Comparison must start from identical complete states"
controls = ["previous-control-quiet", "control-quiet", "repeat-control-quiet"]
if all((args.root / name / "summary.json").exists() for name in controls):
    summaries = {name: read(args.root / name / "summary.json") for name in controls}
    means = [s["mean_ms"] for s in summaries.values()]
    report["default_compatibility"] = {
        "all_final_states_identical": len({
            (args.root / name / "final-state.json").read_bytes() for name in controls
        }) == 1,
        "mean_ms": {name: s["mean_ms"] for name, s in summaries.items()},
        "p95_ms": {name: s["p95_ms"] for name, s in summaries.items()},
        "timing_repeat_variation_ratio": max(means) / min(means),
        "performance_inconclusive_due_to_repeat_variability": max(means) > 1.1 * min(means),
    }
for name in ["synthetic-ee", "synthetic-pf", "dress-ee", "dress-pf"]:
    p = read(args.root / f"{name}.json")
    report["transitions"][name] = {
        "sample_count": len(p["samples"]), "bounds_identical": p["bounds_identical"],
        "maximum_absolute_refresh_delta_j": max(abs(s["candidate_refresh_delta_j"]) for s in p["samples"]),
    }
samples = read(args.root / "slide.json")["samples"]
for mode in ["baseline", "primitive"]:
    energies = [s[f"{mode}_j"] for s in samples]
    report["slide"][mode] = {
        "minimum_j": min(energies), "maximum_j": max(energies),
        "maximum_tangential_force_n": max(abs(s[f"{mode}_force_n"][0]) for s in samples),
    }
report["promotion_gate"] = {
    "planar_tangential_tolerance_n": 1e-6,
    "candidate_planar_sliding_passes": report["slide"]["primitive"]["maximum_tangential_force_n"] <= 1e-6,
    "promoted": False,
    "reason": "Artificial tangential force on a frictionless plane; contact-law ablation only",
}
for name in ["vbd-control", "vbd-primitive"]:
    lines = (args.root / name / "convergence.jsonl").read_text(encoding="utf-8").splitlines()
    delta = [d for line in lines for d in json.loads(line)["vbd_contact_refresh_delta_j"]]
    report["transitions"][name] = {
        "refresh_count": len(delta), "min_delta_j": min(delta),
        "max_delta_j": max(delta), "sum_absolute_delta_j": sum(map(abs, delta)),
    }
(args.root / "comparison.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
if args.plot:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, axes = plt.subplots(2, 1, figsize=(8, 6), sharex=True, layout="constrained")
    x = [s["x_m"] * 1000 for s in samples]
    for mode, label, color in [("baseline", "Existing OGC law", "#2673b8"),
                                ("primitive", "Finite-primitive experiment", "#bf3e34")]:
        axes[0].plot(x, [s[f"{mode}_j"] for s in samples], label=label, color=color)
        axes[1].plot(x, [s[f"{mode}_force_n"][0] for s in samples], color=color)
    axes[0].set_ylabel("Contact energy (J)")
    axes[1].set_ylabel("Tangential force (N)")
    axes[1].set_xlabel("Point position across the triangle diagonal (mm)")
    axes[0].legend(frameon=False)
    fig.suptitle("Planar sliding at a constant 1 mm gap\nContinuous contact can still produce artificial resistance")
    for ax in axes:
        ax.grid(alpha=0.2)
    args.plot.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(args.plot, dpi=160)
    plt.close(fig)
print(json.dumps(report, indent=2))
