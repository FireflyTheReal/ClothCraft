"""Summarize the opt-in hard-contact probe; incomplete runs are never successes."""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("results", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    runs = json.loads(args.results.read_text(encoding="utf-8"))
    working = [r for r in runs if r["mode"] == "WorkingSet"]
    controls = [r for r in runs if r["mode"] == "AllLinearized"]
    reference = [r for r in runs if r["mode"] == "Plane"]
    valid = len(working) == 16 and len(reference) == 4 and len(controls) == 16
    for run in working + reference:
        valid &= (
            run["failure"] is None
            and len(run["frames"]) == 480
            and run["unconverged_steps"] == 0
            and run["minimum_swept_gap_m"] >= 0.001 - 1e-11
            and run["max_post_release_energy_rise_j"] < 1e-10
            and run["max_reference_position_error_m"] < 1e-8
            and run["max_reference_velocity_error_m_s"] < 1e-8
        )
    summary = {
        "date": "2026-10-20",
        "scope": "CPU hard-contact QP and isolated planar point/rigid-wire release gates",
        "production_changed": False,
        "gpu_used": False,
        "dress_trajectory_run": False,
        "full_cloth_backend": False,
        "hard_clearance_m": 0.001,
        "ogc_radius_m": 0.003,
        "step_s": 1 / 60,
        "normal_load_n": 225,
        "mass_kg": 0.02,
        "working_set_gate_passed": bool(valid),
        "working_set_runs": len(working),
        "reference_runs": len(reference),
        "working_set_completed_steps": sum(len(r["frames"]) for r in working),
        "max_reference_position_error_m": max(r["max_reference_position_error_m"] for r in working),
        "max_reference_velocity_error_m_s": max(r["max_reference_velocity_error_m_s"] for r in working),
        "min_swept_clearance_m": min(r["minimum_swept_gap_m"] for r in working),
        "max_post_release_energy_rise_j": max(r["max_post_release_energy_rise_j"] for r in working),
        "all_linearized_control_runs": len(controls),
        "all_linearized_incomplete_runs": sum(len(r["frames"]) != 480 for r in controls),
        "all_linearized_failures": [
            {
                key: r[key]
                for key in ("fixture", "cells_per_axis", "row_copies", "held_release", "failure")
            } | {"completed_frames": len(r["frames"])}
            for r in controls if r["failure"] is not None
        ],
        "working_set_final_speeds_m_s": [
            {
                "fixture": r["fixture"],
                "held": r["held_release"],
                "cells": r["cells_per_axis"],
                "copies": r["row_copies"],
                "speed": sum(v * v for v in r["frames"][-1]["velocity"]) ** 0.5,
            } for r in working
        ],
        "timing_scope": "Development binary, isolated point/rigid-wire CPU probe only; not cloth FPS or production step timing",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in summary.items() if k not in (
        "all_linearized_failures", "working_set_final_speeds_m_s"
    )}, indent=2))
    if not valid:
        raise SystemExit("Working-set quality gate failed; inspect raw trajectories")


if __name__ == "__main__":
    main()
