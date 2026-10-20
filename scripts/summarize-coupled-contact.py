"""Matched small-patch traces. Motion and energy are separate diagnostics."""
import argparse
import json
import math
from pathlib import Path


def norm(v):
    return math.sqrt(sum(x * x for x in v))


def delta(a, b):
    return [x - y for x, y in zip(a, b)]


def summarize(run):
    frames = run["frames"]
    release = 180 if run["held"] else 120
    tail = frames[-60:]
    changes = [
        norm(delta(v, old_v)) ** 2
        for before, after in zip(tail, tail[1:])
        for old_v, v in zip(before["v"], after["v"])
    ]
    momenta = [f["momentum"] for f in frames if f["step"] >= release]
    drift = max((norm([p[0] - momenta[0][0], p[2] - momenta[0][2]])
                 for p in momenta), default=0)
    return {
        "method": run["method"],
        "ease": run["ease"],
        "held": run["held"],
        "completed_steps": len(frames),
        "failure": run["failure"],
        "unconverged_steps": sum(not f["converged"] for f in frames),
        "mean_step_ms": run["elapsed_ms"] / max(1, len(frames)),
        "mean_qp_sweeps": sum(f["qp_sweeps"] for f in frames) / max(1, len(frames)),
        "active_block_steps": sum(f["active_block_steps"] for f in frames),
        "singular_block_fallbacks": sum(f["singular_blocks"] for f in frames),
        "max_residual_n": max(f["residual_n"] for f in frames),
        "max_complementarity_j": max(f["complementarity_j"] for f in frames),
        "minimum_height_m": min(f["min_height_m"] for f in frames),
        "max_post_release_energy_rise_j": run["max_post_release_energy_rise_j"],
        "last_material_j": frames[-1]["material_j"],
        "last_kinetic_j": frames[-1]["kinetic_j"],
        "last_relative_kinetic_j": frames[-1]["relative_kinetic_j"],
        "tail_velocity_change_rms_m_s": math.sqrt(sum(changes) / len(changes)),
        "post_release_horizontal_momentum_drift_kg_m_s": drift,
    }


def main():
    p = argparse.ArgumentParser()
    p.add_argument("coordinate", type=Path)
    p.add_argument("active", type=Path)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    base = json.loads(args.coordinate.read_text())
    active = json.loads(args.active.read_text())
    summaries = [summarize(r) for r in base + active]
    passed = len(base) == len(active) == 4
    for s in summaries:
        passed &= (
            s["completed_steps"] == 360 and s["failure"] is None
            and s["unconverged_steps"] == 0
            and s["max_residual_n"] < 1e-7
            and s["max_complementarity_j"] < 1e-12
            and s["minimum_height_m"] >= 0.001 - 1e-11
            and s["max_post_release_energy_rise_j"] < 1e-10
        )
    comparisons = []
    for a, b in zip(base, active):
        if (a["ease"], a["held"]) != (b["ease"], b["held"]):
            raise SystemExit("Mismatched scenarios")
        errors = {}
        for key in ("x", "v"):
            errors[key] = max(norm(delta(p, q)) for f, g in zip(a["frames"], b["frames"])
                              for p, q in zip(f[key], g[key]))
        passed &= errors["x"] < 1e-6 and errors["v"] < 1e-5
        comparisons.append({
            "ease": a["ease"], "held": a["held"],
            "max_position_difference_m": errors["x"],
            "max_velocity_difference_m_s": errors["v"],
            "observed_time_ratio": a["elapsed_ms"] / b["elapsed_ms"],
        })
    result = {
        "scope": "12-vertex sewn cloth, production material/sewing energy, hard floor contact; CPU development build",
        "quality_gate_passed": bool(passed),
        "production_changed": False,
        "whole_dress_validated": False,
        "coupled_deforming_self_contact_validated": False,
        "timing_caveat": "One sequential sample per method/scenario; excludes trace serialization, not production cloth FPS",
        "runs": summaries, "comparisons": comparisons,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    if not passed:
        raise SystemExit("Coupled-contact gate failed; preserve the evidence")


if __name__ == "__main__":
    main()
