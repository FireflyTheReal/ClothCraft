"""Summarize matched seam_block_probe traces (CPU only, standard library).

Highpass motion includes physical acceleration: it is not a standalone jitter
verdict. A higher-iteration trajectory is a convergence comparator, not truth.
Annotations contain transient indices and must come from the same compiled mesh.
"""
import argparse
import json
import math
from contextlib import ExitStack
from pathlib import Path
from statistics import fmean


def norm2_difference(a, b):
    return sum((x - y) ** 2 for x, y in zip(a, b, strict=True))


def analyze(folder, annotations=None, reference=None):
    meta = json.loads((folder / "summary.json").read_text())
    if meta["steps"] <= 60:
        raise ValueError("Analysis needs more than 60 steps")
    if reference is None and (folder / "reference/trajectory.jsonl").exists():
        reference = folder / "reference/trajectory.jsonl"
    if reference is not None:
        controls = reference.parent.parent / "controls.jsonl"
        if controls.read_bytes() != (folder / "controls.jsonl").read_bytes():
            raise ValueError("Reference and candidate controls differ")
    annotation_file = annotations or folder / "annotations.json"
    groups = json.loads(annotation_file.read_text())["seams"] if annotation_file.exists() else []
    dt = meta["settings"][0]["dt"]
    modes = []
    for result in meta["results"]:
        mode = result["mode"]
        rows = [json.loads(line) for line in (folder / mode / "energy.jsonl").read_text().splitlines()]
        metrics = [json.loads(line) for line in (folder / mode / "metrics.jsonl").read_text().splitlines()]
        phase_stats = {}
        sums = {}
        count = {}
        error = {}
        with ExitStack() as stack:
            trace = stack.enter_context((folder / mode / "trajectory.jsonl").open())
            ref = stack.enter_context(reference.open()) if reference else None
            previous = json.loads(next(trace))
            if ref and json.loads(next(ref)) != previous:
                raise ValueError("Reference initial state differs")
            for step, line in enumerate(trace):
                current = json.loads(line)
                comparison = json.loads(next(ref)) if ref else None
                if comparison and abs(current["time"] - comparison["time"]) > 1e-10:
                    raise ValueError("Reference timing differs")
                phase = rows[step]["phase"]
                if step >= 60:
                    values = sums.setdefault(phase, [0.0] * len(current["v"]))
                    for i, (a, b) in enumerate(zip(current["v"], previous["v"], strict=True)):
                        values[i] += norm2_difference(a, b) * (0.5 * dt) ** 2
                    count[phase] = count.get(phase, 0) + 1
                    if comparison:
                        error[phase] = error.get(phase, 0.0) + sum(norm2_difference(a, b) for a, b in zip(current["x"], comparison["x"], strict=True))
                previous = current
            if step + 1 != len(rows):
                raise ValueError("Truncated trace")
            if ref and next(ref, None) is not None:
                raise ValueError("Reference length differs")
        for phase, values in sums.items():
            sample = [r for r in rows[60:] if r["phase"] == phase]
            n = count[phase]
            a = [r["attachment_m"] for r in sample]
            phase_stats[phase] = {
                "samples": n,
                "highpass_rms_m": math.sqrt(fmean(values) / n),
                "maximum_vertex_highpass_rms_m": math.sqrt(max(values) / n),
                "seams": {g["id"]: math.sqrt(fmean(values[i] for i in g["vertices"]) / n) for g in groups if g["vertices"]},
                "position_rms_to_reference_m": math.sqrt(error[phase] / (n * len(values))) if phase in error else None,
                "attachment_path_length_m": sum(math.sqrt(norm2_difference(x, y)) for x, y in zip(a, a[1:])),
                "attachment_net_displacement_m": math.sqrt(norm2_difference(a[0], a[-1])),
                "total_energy_end_minus_start_j": sample[-1]["total_j"] - sample[0]["total_j"],
                "positive_energy_steps": sum(y["total_j"] - x["total_j"] > 1e-12 for x, y in zip(sample, sample[1:])),
            }
        modes.append({**result, "phases": phase_stats,
            "mean_accepted_blocks": fmean(r["seam_blocks"]["accepted"] for r in metrics[60:]),
            "mean_objective_decrease_j": fmean(r["seam_blocks"]["objective_decrease_j"] for r in metrics[60:])})
    return {"source": str(folder), "gpu_used": meta["gpu_used"], "vertices": meta["vertices"], "steps": meta["steps"], "warmup_steps": 60, "results": modes,
        "limits": "Highpass contains physical acceleration. Phase energy changes during pulling/hold/release do not remove control work. Annotations require identical mesh indexing. Timings are interleaved full-step measurements excluding offline audits, not native-window throughput."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--annotations", type=Path)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    text = json.dumps(analyze(args.folder, args.annotations, args.reference), indent=2) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text, end="")


if __name__ == "__main__":
    main()
