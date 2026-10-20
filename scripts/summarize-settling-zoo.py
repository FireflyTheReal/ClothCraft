"""Compact evidence from CPU settling_zoo runs; does not modify their traces."""
import argparse
import json
from pathlib import Path


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def summarize(root):
    manifest = read(root / "manifest.json")
    cases = read(root / "summary.json")
    result = {"run": root.name, "manifest": manifest, "cases": {}}
    provenance = root / "provenance.json"
    if provenance.exists():
        result["binary_sha256"] = read(provenance)["binary"]["Hash"]
    for name, modes in cases.items():
        case = result["cases"][name] = {"modes": modes}
        continuous = root / name / "continuous/final-state.json"
        fresh = root / name / "fresh-contacts/final-state.json"
        if continuous.exists() and fresh.exists():
            case["same_final_state_after_contact_rebuild"] = read(continuous) == read(fresh)
        for mode, summary in modes.items():
            log = root / name / mode / "observations.jsonl"
            rows = [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]
            summary["accepted_steps"] = len(rows)
            summary["first_20_steps_peak_mass_rms_speed_m_s"] = max(
                row["mass_rms_speed_m_s"] for row in rows[:20])
            summary["first_20_steps_fixed_energy_change_j"] = sum(
                row["audit"]["delta_mechanical_j"] for row in rows[:20])
            summary["first_20_steps_contact_refresh_j"] = sum(
                row["contact_refresh_j"] for row in rows[:20])
            if mode == "pull-release":
                release = [row for row in rows if row["phase"] == "release"]
                summary["early_release_peak_speed_m_s"] = max(
                    row["max_speed_m_s"] for row in release[:120])
                summary["late_release_peak_speed_m_s"] = max(
                    row["max_speed_m_s"] for row in release[-120:])
                summary["control_anchor"] = read(root / name / mode / "control-anchor.json")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("roots", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    results = [summarize(root) for root in args.roots]
    args.output.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    for run in results:
        print(run["run"])
        for name, case in run["cases"].items():
            for mode in ("continuous", "cold-restart", "pull-release"):
                r = case["modes"][mode]
                print(f"  {name:16} {mode:13} {r['classification']:30} "
                      f"span={r['tail_max_vertex_aabb_diagonal_m'] * 1e6:.3f} um "
                      f"force={r['tail_physical_force_rms_n']['mean']:.3g} N "
                      f"step={r['tail_step_ms']['mean']:.2f} ms")


if __name__ == "__main__":
    main()
