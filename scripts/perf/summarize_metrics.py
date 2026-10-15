#!/usr/bin/env python3
"""Summarise a `metrics.jsonl` written by cloth-fit or motion_probe.

One JSON object per line, one line per simulated step. Nested objects are
flattened one level (`contact_profile.refit_ms`), which is how the OGC
detection breakdown is reported.

Warmup steps are excluded from the statistics but reported, because the first
steps of a fresh scene are an assembly transient, not steady state.

Usage:
    scripts/perf/summarize_metrics.py [--warmup 60] [--json] FILE [FILE ...]

The output is deliberately dependency-free (standard library only) so it runs
anywhere Python 3 does. Raw numbers stay in the file; this only summarises.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def flatten(row: dict, prefix: str = "") -> dict:
    out: dict = {}
    for key, value in row.items():
        name = f"{prefix}{key}"
        if isinstance(value, dict):
            out.update(flatten(value, f"{name}."))
        elif isinstance(value, bool) or value is None:
            continue
        elif isinstance(value, (int, float)):
            out[name] = float(value)
    return out


def percentile(values: list[float], fraction: float) -> float:
    """Nearest-rank percentile; no interpolation, so every printed value is a
    step that was actually measured."""
    if not values:
        return float("nan")
    ordered = sorted(values)
    index = min(len(ordered) - 1, int(len(ordered) * fraction))
    return ordered[index]


def summarise(path: Path, warmup: int) -> dict:
    rows: list[dict] = []
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                rows.append(flatten(json.loads(line)))

    measured = rows[warmup:]
    fields = sorted({key for row in measured for key in row})
    stats: dict = {
        "file": str(path),
        "steps_total": len(rows),
        "steps_measured": len(measured),
        "warmup_excluded": min(warmup, len(rows)),
        "fields": {},
    }
    for field in fields:
        values = [row[field] for row in measured if field in row]
        if not values:
            continue
        stats["fields"][field] = {
            "n": len(values),
            "mean": sum(values) / len(values),
            "p50": percentile(values, 0.50),
            "p95": percentile(values, 0.95),
            "max": max(values),
            "min": min(values),
        }
    return stats


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--warmup", type=int, default=0)
    parser.add_argument("--json", action="store_true", help="emit one JSON object per file")
    parser.add_argument(
        "--fields",
        default="",
        help="comma-separated field whitelist (default: every numeric field)",
    )
    args = parser.parse_args()

    results = []
    for path in args.files:
        if not path.is_file():
            print(f"missing: {path}", file=sys.stderr)
            return 1
        stats = summarise(path, args.warmup)
        results.append(stats)
        if args.json:
            continue
        print(f"# {path}  ({stats['steps_measured']} measured of {stats['steps_total']} steps)")
        wanted = {f for f in args.fields.split(",") if f}
        print(f"{'field':<34}{'n':>6}{'mean':>12}{'p50':>12}{'p95':>12}{'max':>12}")
        for field, values in stats["fields"].items():
            if wanted and field not in wanted:
                continue
            if values["max"] == 0.0 and values["mean"] == 0.0:
                continue
            print(
                f"{field:<34}{values['n']:>6}{values['mean']:>12.4f}"
                f"{values['p50']:>12.4f}{values['p95']:>12.4f}{values['max']:>12.4f}"
            )
        print()

    if args.json:
        json.dump(results, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
