"""Offline independent/mathematical reference fixtures; no runtime dependency."""
import json
from pathlib import Path

import numpy as np
import scipy
from scipy.optimize import LinearConstraint, minimize


def main():
    directory = Path(__file__).resolve().parents[1] / "tests/fixtures"
    rng = np.random.default_rng(24680)
    cases = []
    for _ in range(12):
        n = 6
        a = rng.normal(size=(10, n)) * np.geomspace(.01, 100, n)
        c = rng.normal(size=(8, n))
        c /= np.linalg.norm(c, axis=1)[:, None]
        x = rng.normal(size=n)
        multipliers = np.r_[rng.uniform(.1, 2, 2), np.zeros(6)]
        d = c @ x
        d[2:] -= rng.uniform(.2, 2, 6)
        residual = np.linalg.lstsq(a.T, c.T @ multipliers, rcond=None)[0]
        b = a @ x - residual
        if np.linalg.norm(a.T @ (a @ x - b) - c.T @ multipliers) >= 1e-9:
            raise RuntimeError("Manufactured KKT reference is inaccurate")
        cases.append({
            "a": a.ravel().tolist(), "b": b.tolist(), "solution": x.tolist(),
            "initial": rng.normal(size=n).tolist(),
            "rows": [{"jacobian": v.tolist(), "offset": float(w)} for v, w in zip(c, d)],
            "multipliers": multipliers.tolist(),
        })
    data = {
        "seed": 24680,
        "scope": "Manufactured convex KKT optima, full-column-rank A, known nonnegative active multipliers",
        "cases": cases,
    }
    (directory / "constrained-ls-kkt.json").write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")

    p = json.loads((directory / "bounded-contact-ls.json").read_text(encoding="utf-8"))
    n, free = p["columns"], p["free_prefix"]
    a, b = np.array(p["a"]).reshape(-1, n), np.array(p["b"])
    c = np.array([r["jacobian"] + [0.] * (n - free) for r in p["geometry"]] + np.eye(n)[free:].tolist())
    d = np.r_[[r["offset"] for r in p["geometry"]], np.zeros(n - free)]
    unit = .001
    weight = np.linalg.norm(a @ np.array(p["initial"]) - b)
    aa, bb = a * unit / weight, b / weight
    result = minimize(
        lambda z: .5 * np.linalg.norm(aa @ z - bb) ** 2,
        np.array(p["initial"]) / unit,
        jac=lambda z: aa.T @ (aa @ z - bb),
        constraints=LinearConstraint(c, d / unit, np.inf),
        method="SLSQP", options={"maxiter": 2000, "ftol": 1e-14},
    )
    if not result.success:
        raise RuntimeError(result.message)
    x = result.x * unit
    data = {
        "scipy": scipy.__version__, "success": bool(result.success),
        "message": str(result.message), "iterations": int(result.nit),
        "residual_norm": float(np.linalg.norm(a @ x - b)),
        "minimum_raw_gap": float(np.min(c @ x - d)), "solution": x.tolist(),
    }
    (directory / "constrained-contact-ls-oracle.json").write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
