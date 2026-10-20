"""Quality-first analysis. Different timesteps/iterations are not speed rankings."""
import argparse
import hashlib
import json
from pathlib import Path


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--plot", action="store_true")
    args = parser.parse_args()
    rows = []
    histories = {}
    for path in sorted(args.root.glob("*/summary.json")):
        r = read(path)
        p = read(path.parent / "protocol.json")
        last = r["last"]
        m = last["metrics"]
        history = [json.loads(line) for line in (path.parent / "steps.jsonl").read_text().splitlines()]
        histories[path.parent.name] = (p, history)
        traces = path.parent / "convergence.jsonl"
        sweeps = [s for line in traces.read_text().splitlines() for s in json.loads(line)["sweeps"]]
        rows.append({
            "run": path.parent.name, "backend": r["backend"],
            "initial_state_sha256": hashlib.sha256((path.parent / "initial-state.json").read_bytes()).hexdigest(),
            "dt_s": p["settings"]["dt"], "iterations": p["settings"]["iterations"],
            "accepted_steps": r["accepted_steps"], "requested_steps": r["requested_steps"],
            "physical_seconds": r["accepted_steps"] * p["settings"]["dt"],
            "failure": r["failure"], "final_intersection_check": r["final_intersection_check"],
            "audit_every_step": p.get("audit_every_step", False),
            "seam_gap_mm": 1000 * m["max_seam_gap_m"],
            "stretch_p95_percent": m["extension_p95_percent"],
            # VBD's internal residual is assembled during local updates. Do not
            # mistake it for the common fresh final residual used by lab solvers.
            "fresh_target_residual_rms_n": m["residual_rms_n"] if r["backend"] != "Vbd" else None,
            "minimum_distance_m": min(s["fresh_minimum_distance_m"] for s in history),
            "last_panel_speed": last["panel_speed"],
            "final_full_stiffness_fraction": sweeps[-1]["full_stiffness_fraction"] if sweeps else None,
            "max_contact_refresh_jump_j": max((s["contact_refresh_delta_j"] for s in sweeps), default=None),
        })
    interactions = []
    for name, (protocol, history) in histories.items():
        if not name.endswith("-pull"):
            continue
        quiet = histories.get(name.removesuffix("-pull") + "-quiet")
        if not quiet:
            continue
        response = []
        for a, b in zip(history, quiet[1]):
            response.append(1000 * sum((x - y)**2 for x, y in zip(a["grab_position"], b["grab_position"]))**0.5)
        release = protocol["pull_start"] + protocol["pull_steps"] + protocol["hold_steps"]
        interactions.append({"run": name, "peak_response_vs_quiet_mm": max(response),
                             "response_at_release_mm": response[min(release, len(response)-1)],
                             "final_response_vs_quiet_mm": response[-1]})
    result = {"runs": rows, "interactions": interactions,
              "note": "No settling or performance winner inferred. Each pull uses the same world-space target. One disconnected panel falls; speeds are per panel."}
    (args.root / "quality.json").write_text(json.dumps(result, indent=2) + "\n")
    for row in rows:
        print(f'{row["run"]:28} gap {row["seam_gap_mm"]:8.2f} mm, stretch {row["stretch_p95_percent"]:8.2f}%, steps {row["accepted_steps"]}/{row["requested_steps"]}')
    if args.plot:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        fig, axes = plt.subplots(1, 2, figsize=(12, 4.4), layout="constrained")
        colors = {"Xpbd":"#1f77b4", "AvbdFinite":"#ff7f0e", "XpbdPrimal":"#2ca02c", "AvbdTarget":"#d62728"}
        for algorithm in ("Xpbd", "AvbdFinite", "XpbdPrimal", "AvbdTarget"):
            subset = sorted((r for r in rows if r["backend"] == algorithm and r["run"].startswith("single-")), key=lambda r: r["iterations"])
            if subset:
                axes[0].loglog([r["iterations"] for r in subset], [r["fresh_target_residual_rms_n"] for r in subset], "o-", label=algorithm, color=colors[algorithm])
        axes[0].set(xlabel="Nonlinear sweeps (different work per sweep)", ylabel="Fresh final residual RMS (N)", title="Same saved dress, one physical step")
        axes[0].legend(fontsize=8)
        for name, (p, history) in histories.items():
            if name.endswith("-pull"):
                color = {"vbd-pull":"#333333", "avbd-target-pull":"#d62728", "xpbd-primal-pull":"#2ca02c", "xpbd-small-pull":"#9467bd"}[name]
                axes[1].semilogy([(i+1)*p["settings"]["dt"] for i in range(len(history))], [1000*s["metrics"]["max_seam_gap_m"] for s in history], label=name.removesuffix("-pull"), color=color)
        axes[1].axvspan(0.5, 1.25, color="grey", alpha=0.12, label="Pull / hold")
        axes[1].set(xlabel="Physical time (s)", ylabel="Maximum seam gap (mm)", title="Pull, hold, then release")
        axes[1].legend(fontsize=8)
        for ax in axes:
            ax.grid(alpha=0.25)
        fig.savefig(args.root / "quality.png", dpi=160)
        fig.savefig(args.root / "quality.svg")


if __name__ == "__main__":
    main()
