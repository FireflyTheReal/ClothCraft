"""Passive long-tail audit. A quiet metric is not proof of numerical correctness."""
import argparse
import json
from pathlib import Path

import numpy as np


def components(metadata):
    n = metadata["vertices"]
    parent = list(range(n))

    def root(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    for support in metadata["triangles"] + metadata["seam_supports"]:
        if any(not isinstance(i, int) or not 0 <= i < n for i in support):
            raise ValueError("Invalid material connectivity")
        for i in support[1:]:
            parent[root(i)] = root(support[0])
    groups = {}
    for i in range(n):
        groups.setdefault(root(i), []).append(i)
    return list(groups.values())


def rigid_residual(x, v, masses, groups):
    """Mass-weighted least-squares translation + angular velocity per component."""
    x, v, masses = np.asarray(x), np.asarray(v), np.asarray(masses)
    n = len(masses)
    if (x.shape != (n, 3) or v.shape != (n, 3)
            or not np.isfinite(x).all() or not np.isfinite(v).all()
            or not np.isfinite(masses).all() or (masses <= 0).any()):
        raise ValueError("Invalid motion state or mass")
    if sorted(i for ids in groups for i in ids) != list(range(n)):
        raise ValueError("Components must partition the vertices")
    residual = np.zeros_like(v, dtype=float)
    fitted = np.zeros_like(v, dtype=float)
    for ids in groups:
        weights = masses[ids]
        center = np.average(x[ids], axis=0, weights=weights)
        translation = np.average(v[ids], axis=0, weights=weights)
        r = x[ids] - center
        matrix = np.zeros((len(ids), 3, 3))
        matrix[:, 0, 1], matrix[:, 0, 2] = r[:, 2], -r[:, 1]
        matrix[:, 1, 0], matrix[:, 1, 2] = -r[:, 2], r[:, 0]
        matrix[:, 2, 0], matrix[:, 2, 1] = r[:, 1], -r[:, 0]
        sqrt_mass = np.sqrt(weights)[:, None]
        omega = np.linalg.lstsq(
            (matrix * sqrt_mass[:, :, None]).reshape(-1, 3),
            ((v[ids] - translation) * sqrt_mass).ravel(), rcond=1e-12,
        )[0]
        fitted[ids] = translation + matrix @ omega
        residual[ids] = v[ids] - fitted[ids]
    return residual, fitted


def rms(values, masses):
    return float(np.sqrt(np.sum(np.asarray(masses) * np.sum(values * values, axis=-1))
                         / np.sum(masses)))


def summarize(source_path, tail_path, metadata_path, dt, window):
    if not np.isfinite(dt) or dt <= 0 or window < 2:
        raise ValueError("Invalid time/window")
    source = json.loads(source_path.read_text(encoding="utf-8"))
    tail = json.loads(tail_path.read_text(encoding="utf-8"))
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    for key in ["vertices", "sewn_ease", "fold_degrees"]:
        if source[key] != tail[key] or metadata[key] != tail[key]:
            raise ValueError(f"Fixture mismatch: {key}")
    for key in ["held", "force_law", "constrained_newton", "newton_path_repair",
                "pairs", "release_step"]:
        if source.get(key) != tail.get(key):
            raise ValueError(f"Protocol mismatch: {key}")
    first, stop = tail["first_step"], tail["requested_steps"]
    if first < tail["release_step"]:
        raise ValueError("Passive tail starts before external control is released")
    prefix = [f for f in source["frames"] if f["step"] < first]
    if (not prefix or prefix[-1]["step"] != first - 1
            or [f["step"] for f in prefix] != list(range(source["first_step"], first))
            or any(not f["converged"] for f in prefix)):
        raise ValueError("Missing/invalid checkpoint prefix")
    raw_frames = tail["frames"]
    complete = (tail["failure"] is None and first < stop
                and [f["step"] for f in raw_frames] == list(range(first, stop))
                and tail["completed_steps"] == len(raw_frames))
    frames = []
    for frame in raw_frames:
        if not frame["converged"]:
            break
        frames.append(frame)
    physical = bool(complete and len(frames) == len(raw_frames) and all(
        f["converged"] and f["residual_n"] < 1e-7
        and f["complementarity_j"] < 1e-12
        and f["minimum_floor_height_m"] >= .001 - 1e-10
        and f["minimum_pair_distance_m"] >= .001 - 1e-10
        for f in frames))
    previous = prefix[-1]
    energy_finite = bool(np.isfinite([f["energy_j"] for f in [previous] + frames]).all())
    rise = (max([0.] + [b["energy_j"] - a["energy_j"]
                       for a, b in zip([previous] + frames, frames)])
            if energy_finite else None)
    energy_passed = bool(physical and energy_finite and rise < 1e-10)
    masses = np.asarray(metadata["masses_kg"])
    groups = components(metadata)
    history = [rigid_residual(f["x"], f["v"], masses, groups) for f in [previous] + frames]
    samples = []
    for i, frame in enumerate(frames, 1):
        internal, rigid = history[i]
        highpass = .5 * dt * (internal - history[i - 1][0])
        samples.append({
            "step": frame["step"], "nonrigid_speed_rms_m_s": rms(internal, masses),
            "rigid_speed_rms_m_s": rms(rigid, masses),
            "nonrigid_highpass_rms_m": rms(highpass, masses),
        })
    last = samples[-window:]
    metrics = ({key: max(f[key] for f in last) for key in [
        "nonrigid_speed_rms_m_s", "rigid_speed_rms_m_s", "nonrigid_highpass_rms_m"
    ]} if last else None)
    quiet = bool(physical and len(last) == window
                 and metrics["nonrigid_speed_rms_m_s"] < 1e-6
                 and metrics["nonrigid_highpass_rms_m"] < 1e-7)
    return {
        "source": str(source_path), "tail": str(tail_path), "metadata": str(metadata_path),
        "dt_s": dt, "first_step": first, "requested_stop": stop,
        "completed_steps": len(raw_frames), "accepted_prefix_steps": len(frames),
        "excluded_unconverged_or_later_frames": len(raw_frames) - len(frames),
        "failure": tail["failure"],
        "full_tail_complete": complete, "physical_gate_passed": physical,
        "quality_gate_passed": energy_passed,
        "maximum_energy_rise_including_checkpoint_boundary_j": rise,
        "energy_gate_passed": energy_passed,
        "component_sizes": [len(ids) for ids in groups],
        "final_window_steps": len(last), "final_window_maxima": metrics,
        "quiet_diagnostic_passed": bool(quiet and energy_passed),
        "quiet_thresholds": {"nonrigid_speed_m_s": 1e-6, "highpass_m": 1e-7},
        "interpretation": "Quiet thresholds are diagnostics, never solver damping/sleep or proof of no numerical error. Rigid drift is retained separately.",
        "samples": samples,
    }


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--source", type=Path, required=True)
    p.add_argument("--tail", type=Path, required=True)
    p.add_argument("--metadata", type=Path, required=True)
    p.add_argument("--dt", type=float, required=True)
    p.add_argument("--window", type=int, default=60)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    result = summarize(args.source, args.tail, args.metadata, args.dt, args.window)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "samples"}, indent=2))


if __name__ == "__main__":
    main()
