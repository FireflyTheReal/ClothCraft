"""Regenerate the public linear-search oracle, not garment data.

Requires NumPy/SciPy only for offline reference generation. Rust runtime and
the test suite consume the checked-in JSON without a Python dependency.
"""
import json
from pathlib import Path

import numpy as np
import scipy
from scipy.optimize import lsq_linear


def main():
    rng = np.random.default_rng(117083)
    cases = []
    for _ in range(12):
        a = rng.normal(size=(14, 8)) * np.geomspace(1e-4, 1e4, 8)
        b = rng.normal(size=14)
        lower = np.r_[[-np.inf] * 3, [0.0] * 5]
        result = lsq_linear(
            a, b, bounds=(lower, np.inf), method="bvls", tol=1e-12, max_iter=256
        )
        if not result.success:
            raise RuntimeError(result.message)
        initial = np.r_[rng.normal(size=3), rng.random(5)]
        cases.append({
            "a": a.ravel().tolist(), "b": b.tolist(),
            "initial": initial.tolist(), "solution": result.x.tolist(),
            "residual_norm": float(np.linalg.norm(a @ result.x - b)),
        })
    result = {
        "scipy": scipy.__version__, "numpy": np.__version__,
        "seed": 117083, "cases": cases,
    }
    destination = Path(__file__).resolve().parents[1] / "tests/fixtures/bounded-ls-oracle.json"
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
