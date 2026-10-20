"""Aggregate local snapshot audits; do not copy private geometry or feature IDs."""
import argparse
import hashlib
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("root", type=Path)
args = parser.parse_args()
report = {
    "scope": "CPU snapshot assembly audit; no new simulation or installed force law",
    "runs": {},
}
for name in ["forest-checkpoint", "forest-after-release"]:
    path = args.root / (name + ".json")
    data = json.loads(path.read_text(encoding="utf-8"))
    rows = []
    for sample in data["results"]:
        rows.append({key: value for key, value in sample["audit"].items()
                     if not key.startswith("worst_")})
    report["runs"][name] = {
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "state_time_s": data["state_time_s"],
        "cloth_vertices": data["cloth_vertices"],
        "total_vertices": data["total_vertices"],
        "triangles": data["triangles"],
        "radii": rows,
    }
log = (args.root / "tests.log").read_text(encoding="utf-8-sig")
report["counterexample_log_lines"] = [
    line for line in log.splitlines()
    if line.startswith("epsilon=") or line.startswith("dense-cycle force")
]
regression = (args.root / "regression-tests.log").read_text(encoding="utf-8-sig")
report["regression_tests_passed"] = sum(
    map(int, re.findall(r"test result: ok\. (\d+) passed", regression))
)
report["regression_failures_reported"] = "test result: FAILED" in regression
for name in ["cpu", "desktop"]:
    log = (args.root / f"clippy-{name}.log").read_text(encoding="utf-8-sig")
    report[f"clippy_{name}_completion_marker"] = "Finished" in log and "error:" not in log
output = args.root / "summary.json"
output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps({key: value for key, value in report.items() if key != "runs"}, indent=2))
