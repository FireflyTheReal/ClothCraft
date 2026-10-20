"""Summarize headless cloth-fit checkpoints without changing any state.

Usage: python scripts/summarize-drape.py OUTPUT_DIRECTORY
Each child run must contain run.json, metrics.jsonl and fitting-project.json.
Connected components use panel/seam IDs; mesh indices are read only from the
matching run's OBJ and are never saved into an editable document.
"""
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def groups(path):
    result = {}
    current = None
    for line in path.read_text().splitlines():
        if line.startswith("g "):
            current = line[2:]
            result[current] = set()
        elif line.startswith("f "):
            result[current].update(int(v.split("/")[0]) - 1 for v in line[2:].split())
    return result


def components(template):
    graph = {p["id"]: set() for p in template["panels"]}
    for seam in template["seams"]:
        ids = {r["panel"] for r in [seam["a"], seam["b"], *seam.get("a_tail", []), *seam.get("b_tail", [])]}
        for p in ids:
            graph[p].update(ids - {p})
    remaining = set(graph)
    while remaining:
        pending = [min(remaining)]
        found = set()
        while pending:
            p = pending.pop()
            if p in found:
                continue
            found.add(p)
            pending.extend(graph[p] - found)
        remaining -= found
        yield sorted(found)


def summarize(run):
    report = read(run / "run.json")
    project = read(run / "fitting-project.json")
    state = project["state"]
    initial = [[float(x) for x in line[2:].split()] for line in (run / "initial.obj").read_text().splitlines() if line.startswith("v ")]
    panel_vertices = groups(run / "cloth.obj")
    rows = [json.loads(line) for line in (run / "metrics.jsonl").read_text().splitlines()]
    dt = report["settings"]["dt"]
    tail = rows[-max(1, math.ceil(5 / dt)):]
    costs = sorted(r["step_ms"] for r in tail)
    result = {
        "checkpoint_sha256": hashlib.sha256((run / "fitting-project.json").read_bytes()).hexdigest(),
        "state_time_s": state["time"], "physical_duration_s": len(rows) * dt,
        "settings": report["settings"],
        "classification": "Draped checkpoint with preserved velocities; equilibrium is not certified",
        "restart": "Prediction, motion and contact caches are rebuilt; use identical warmup for comparisons",
        "components": [],
    }
    if tail:
        result["tail_seconds"] = len(tail) * dt
        result["tail_step_ms"] = {"mean": statistics.mean(costs), "p95": costs[math.ceil(.95 * len(costs)) - 1], "max": max(costs)}
        result["tail_ms_per_1_60s"] = statistics.mean(costs) / (60 * dt)
        result["tail_motion"] = {k: statistics.mean(r["motion"][k] for r in tail) for k in ["speed_rms_m_s", "highpass_rms_m", "oscillating_vertices"]}
        result["tail_seam_gap_max_m"] = max(r["max_seam_gap_m"] for r in tail)
        result["tail_prestep_min_separation_m"] = min(r["minimum_distance_m"] for r in tail)
    for names in components(project["template"]):
        ids = set().union(*(panel_vertices[name] for name in names))
        rms = math.sqrt(sum(sum(z*z for z in state["v"][i]) for i in ids) / len(ids))
        drift = [sum(state["x"][i][k] - initial[i][k] for i in ids) / len(ids) for k in range(3)]
        result["components"].append({"panels": names, "vertices": len(ids), "final_speed_rms_m_s_unweighted": rms, "centroid_displacement_m": drift})
    return result


def main():
    root = Path(sys.argv[1])
    results = {p.name: summarize(p) for p in sorted(root.iterdir()) if p.is_dir() and (p / "run.json").exists()}
    if not results:
        raise SystemExit("No completed cloth-fit runs found")
    destination = root / "drape-summary.json"
    destination.write_text(json.dumps(results, indent=2, allow_nan=False) + "\n")
    print(destination)
    for name, row in results.items():
        print(name, row.get("tail_ms_per_1_60s"), "ms / 1/60 s; component speeds:", [(c["panels"], round(c["final_speed_rms_m_s_unweighted"], 6)) for c in row["components"]])


if __name__ == "__main__":
    main()
