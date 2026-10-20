"""Report every result, including completed trajectories with failed solves."""
import argparse
import json
from pathlib import Path


def summarize(path):
    r = json.loads(path.read_text())
    frames = r["frames"]
    complete = (r["completed_steps"] == r["requested_steps"] == len(frames) == 180
                and [f["step"] for f in frames] == list(range(180))
                and r["failure"] is None)
    failed = [f["step"] for f in frames if not f["converged"]]
    floor = min((f["minimum_floor_height_m"] for f in frames), default=None)
    gap = min((f["minimum_pair_distance_m"] for f in frames), default=None)
    force = max((f["residual_n"] for f in frames), default=None)
    comp = max((f["complementarity_j"] for f in frames), default=None)
    release = r.get("release_step", 130 if r["held"] else 100)
    # Include the very first unforced step, including older traces whose
    # root-level diagnostic started one step too late.
    energy_rise = max(
        [0.0] + [
            current["energy_j"] - previous["energy_j"]
            for previous, current in zip(frames, frames[1:])
            if current["step"] >= release
        ]
    )
    geometry = floor is not None and floor >= 0.001 - 1e-10 and gap >= 0.001 - 1e-10
    quality = (complete and not failed and geometry and force < 1e-7
               and comp < 1e-12 and energy_rise < 1e-10)
    balance = [f["energy_balance"] for f in frames if f.get("energy_balance") is not None]
    release_audit = [f for f in frames if f["step"] >= release and f.get("energy_balance") is not None]
    peak = max(release_audit, key=lambda f: f["energy_balance"]["change_j"], default=None)
    return {
        "file": str(path).replace("\\", "/"),
        "held": r["held"],
        "vertices": r["vertices"],
        "pairs": r["pairs"],
        "sewn_ease": r.get("sewn_ease"),
        "fold_degrees": r.get("fold_degrees"),
        "seams": r.get("seams", 0),
        "dual_method": r.get("dual_method", "ActiveBlock"),
        "swept_repair": r.get("swept_repair", False),
        "accumulate_repair": r.get("accumulate_repair"),
        "iterations_budget": r.get("iterations_budget", 96),
        "full_protocol_complete": complete,
        "quality_gate_passed": bool(quality),
        "geometry_gate_passed": bool(geometry),
        "convergence_gate_passed": bool(complete and not failed and force < 1e-7 and comp < 1e-12),
        "energy_decay_gate_passed": bool(complete and energy_rise < 1e-10),
        "energy_audited_steps": len(balance),
        "maximum_energy_balance_closure_error_j": max(
            (abs(b["closure_error_j"]) for b in balance), default=None),
        "largest_post_release_energy_change_step": peak["step"] if peak else None,
        "energy_balance_at_largest_change": peak["energy_balance"] if peak else None,
        "completed_steps": r["completed_steps"],
        "failure": r["failure"],
        "unconverged_steps": len(failed),
        "unconverged_step_ids": failed,
        "minimum_pair_distance_m": gap,
        "minimum_floor_height_m": floor,
        "maximum_force_residual_n": force,
        "maximum_complementarity_j": comp,
        "release_step": release,
        "maximum_post_release_energy_rise_j": energy_rise,
        "last_kinetic_j": frames[-1]["kinetic_j"] if frames else None,
        "last_seam_energy_j": frames[-1].get("seam_energy_j") if frames else None,
        "mean_step_ms": r["elapsed_ms"] / max(1, len(frames)),
        "certificate_rejections": sum(f["certificate_rejections"] for f in frames),
        "maximum_ee_reactions": max((f["max_reaction_counts_vf_ee"][1] for f in frames), default=0),
        "repair_solves": sum(f.get("repair_solves", 0) for f in frames),
        "unconverged_repair_qps": sum(f.get("repair_failures", 0) for f in frames),
        "rank_deficient_block_fallbacks": sum(f["singular_block_fallbacks"] for f in frames),
        "reduced_active_blocks": sum(f.get("reduced_active_blocks", 0) for f in frames),
        "boundary_pivots": sum(f.get("boundary_pivots", 0) for f in frames),
        "schur_entries_computed": sum(f.get("schur_entries_computed", 0) for f in frames),
        "schur_cache_hits": sum(f.get("schur_cache_hits", 0) for f in frames),
        "schur_max_bytes": max((f.get("schur_max_bytes", 0) for f in frames), default=0),
    }


def main():
    p = argparse.ArgumentParser()
    p.add_argument("results", nargs="+", type=Path)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--require-all", action="store_true")
    args = p.parse_args()
    runs = [summarize(path) for path in args.results]
    result = {
        "scope": "Small layered cloth, coupled VF/EE inequalities and swept straight-path certificates; sizes and seams per run",
        "production_changed": False,
        "gpu_used": False,
        "whole_dress_validated": False,
        "all_quality_gates_passed": all(r["quality_gate_passed"] for r in runs),
        "timing_scope": "Single CPU development-build samples; includes diagnostics, not production FPS",
        "runs": runs,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    if args.require_all and not result["all_quality_gates_passed"]:
        raise SystemExit("Quality gate failed; completion/low kinetic energy is not convergence")


if __name__ == "__main__":
    main()
