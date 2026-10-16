"""Compare CPU contact query replays; no dependencies and no GPU execution."""
import argparse
import json
import statistics
from pathlib import Path


def analyze(paths, discard):
    runs = [json.loads(p.read_text(encoding="utf-8")) for p in paths]
    if not runs or any(len(r["samples"]) <= discard for r in runs):
        raise ValueError("Every run needs more samples than the warmup discard")
    fingerprints = [s["fingerprint"] for s in runs[0]["samples"]]
    if any([s["fingerprint"] for s in r["samples"]] != fingerprints for r in runs):
        raise ValueError("Contact/radius fingerprints differ between repeated runs")
    samples = [s for r in runs for s in r["samples"][discard:]]
    return fingerprints, {
        "vertices": runs[0]["vertices"],
        "queries": len(samples),
        "median_ms": statistics.median(s["total_ms"] for s in samples),
        "per_run_median_ms": [statistics.median(s["total_ms"] for s in r["samples"][discard:]) for r in runs],
        "median_candidate_pairs": statistics.median(s["profile"]["candidate_pairs"] for s in samples),
        "median_aabb_rejections": statistics.median(s["profile"].get("aabb_rejections", 0) for s in samples),
        "median_contacts": statistics.median(s["contacts"] for s in samples),
        "rebuilds": sum(s["rebuilt"] for s in samples),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", nargs="+", type=Path, required=True)
    parser.add_argument("--candidate", nargs="+", type=Path, required=True)
    parser.add_argument("--discard", type=int, default=5)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.discard < 0:
        parser.error("discard must be nonnegative")
    a, baseline = analyze(args.baseline, args.discard)
    b, candidate = analyze(args.candidate, args.discard)
    if a != b or baseline["vertices"] != candidate["vertices"]:
        raise ValueError("Candidate changed contact/radius fingerprints or vertex count")
    result = {
        "kind": "CPU contact query replay; not simulation or FPS",
        "equal_fingerprints_all_queries": True,
        "baseline": baseline,
        "candidate": candidate,
        "median_detection_reduction_percent": 100 * (1 - candidate["median_ms"] / baseline["median_ms"]),
    }
    text = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")


if __name__ == "__main__":
    main()
