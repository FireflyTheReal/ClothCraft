"""Summarize the isolated quadrature experiment, not a cloth/FPS benchmark."""
import argparse
import hashlib
import json
import math
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("input", type=Path)
parser.add_argument("--plot", type=Path)
args = parser.parse_args()
data = json.loads(args.input.read_text(encoding="utf-8"))
report = {
    "scope": data["scope"],
    "input_sha256": hashlib.sha256(args.input.read_bytes()).hexdigest(),
    "runs": [],
}
for run in data["runs"]:
    reference = next(r for r in data["runs"] if r["law"] == "Plane" and r["hold"] == run["hold"])
    trace = run["trace"]
    row = {key: value for key, value in run.items() if key != "trace"}
    row["steps"] = len(trace)
    for key in ["position_m", "velocity_m_s"]:
        row["max_reference_error_" + key] = max(
            math.dist(a[key], b[key]) for a, b in zip(trace, reference["trace"])
        )
    release = run["release_step"]
    row["release_velocity_m_s"] = trace[release]["velocity_m_s"]
    row["final_position_m"] = trace[-1]["position_m"]
    row["final_velocity_m_s"] = trace[-1]["velocity_m_s"]
    report["runs"].append(row)
output = args.input.with_suffix(".summary.json")
output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
if args.plot:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, axes = plt.subplots(2, 2, figsize=(12, 7), constrained_layout=True)
    colors = {"Plane": "black", "PrimitiveSum": "#c44536", "FacePair": "#176b45",
              "WireSum": "#c78b15", "WirePair": "#4267ac"}
    extra = sorted({r["law"] for r in data["runs"]} - colors.keys())
    for index, name in enumerate(extra):
        colors[name] = plt.get_cmap("tab10")((index + 4) % 10)
    for run in data["runs"]:
        col = int(run["hold"])
        t = [(p["step"] + 1) * run["dt_s"] for p in run["trace"]]
        style = "--" if run["law"] in ["FacePair", "WirePair"] else "-"
        for row, key in enumerate(["position_m", "velocity_m_s"]):
            axes[row, col].plot(t, [p[key][0] * 1000 for p in run["trace"]],
                               style, color=colors[run["law"]], label=run["law"],
                               linewidth=1.5)
    for col in range(2):
        axes[0, col].set_title("Release " + ("after holding" if col else "while moving"))
        for row in range(2):
            axes[row, col].axvline(4 if col else 3, color="#888888", linestyle=":")
            axes[row, col].set_xlabel("Time (s)")
            axes[row, col].set_ylabel("Position X (mm)" if row == 0 else "Velocity X (mm/s)")
            axes[row, col].grid(alpha=0.2)
    axes[0, 0].legend(fontsize=8, ncol=2)
    fig.suptitle("Isolated contact patch — analytical plane vs primitive sum vs subtraction\n"
                 "No damping/friction; constant 225 N preload. Not a dress benchmark.")
    args.plot.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(args.plot, dpi=150)
print(json.dumps(report, indent=2))
