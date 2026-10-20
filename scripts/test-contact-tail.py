"""CPU-only regression tests for passive trajectory analysis."""
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np

spec = importlib.util.spec_from_file_location("tail", Path(__file__).with_name("summarize-contact-tail.py"))
tail = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tail)


class TailTests(unittest.TestCase):
    def setUp(self):
        self.x = np.array([[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]])
        self.mass = np.array([1., 2., 3., 4.])

    def test_rigid_motion_is_not_internal_jitter(self):
        velocity = [3., -2., 1.] + np.cross([.4, -.2, .7], self.x)
        residual, rigid = tail.rigid_residual(self.x, velocity, self.mass, [[0, 1, 2, 3]])
        self.assertLess(tail.rms(residual, self.mass), 1e-12)
        self.assertGreater(tail.rms(rigid, self.mass), 1.)

    def test_strain_is_retained_and_components_do_not_merge_on_contact(self):
        center = np.average(self.x, axis=0, weights=self.mass)
        expansion = .2 * (self.x - center)
        residual, _ = tail.rigid_residual(self.x, expansion, self.mass, [[0, 1, 2, 3]])
        np.testing.assert_allclose(residual, expansion, atol=1e-12)
        x = np.concatenate([self.x, self.x + [2., 0., 0.]])
        velocity = np.concatenate([np.tile([0., 1., 0.], (4, 1)), np.tile([0., -1., 0.], (4, 1))])
        metadata = {"vertices": 8, "triangles": [[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]], "seam_supports": []}
        groups = tail.components(metadata)
        self.assertEqual(list(map(len, groups)), [4, 4])
        residual, _ = tail.rigid_residual(x, velocity, np.ones(8), groups)
        self.assertLess(tail.rms(residual, np.ones(8)), 1e-12)
        merged, _ = tail.rigid_residual(x, velocity, np.ones(8), [list(range(8))])
        self.assertGreater(tail.rms(merged, np.ones(8)), .1)
        metadata["seam_supports"] = [[2, 4]]
        self.assertEqual(list(map(len, tail.components(metadata))), [8])

    def test_invalid_mass_or_partition_is_rejected(self):
        with self.assertRaises(ValueError):
            tail.rigid_residual(self.x, self.x, [0., 1., 1., 1.], [[0, 1, 2, 3]])
        with self.assertRaises(ValueError):
            tail.rigid_residual(self.x, self.x, self.mass, [[0, 1], [1, 2, 3]])

    def test_checkpoint_energy_and_incomplete_runs_are_not_hidden(self):
        meta = {"vertices": 4, "sewn_ease": 0., "fold_degrees": 20.,
                "masses_kg": self.mass.tolist(), "triangles": [[0, 1, 2], [0, 2, 3]], "seam_supports": []}
        base = {"vertices": 4, "sewn_ease": 0., "fold_degrees": 20., "held": False,
                "force_law": "invariant-discrete", "constrained_newton": True, "pairs": 0, "release_step": 0}
        frame = {"step": 0, "x": self.x.tolist(), "v": np.zeros((4, 3)).tolist(), "converged": True,
                 "residual_n": 0., "complementarity_j": 0., "minimum_floor_height_m": .001,
                 "minimum_pair_distance_m": .001, "energy_j": 1.}
        source = dict(base, first_step=0, frames=[frame])
        result = dict(base, first_step=1, requested_steps=2, completed_steps=1, failure=None,
                      frames=[dict(frame, step=1, energy_j=1.1)])
        with tempfile.TemporaryDirectory() as d:
            paths = [Path(d) / name for name in ["source.json", "tail.json", "metadata.json"]]
            def run():
                for path, data in zip(paths, [source, result, meta]):
                    path.write_text(json.dumps(data), encoding="utf-8")
                return tail.summarize(*paths, 1/60, 2)
            report = run()
            self.assertTrue(report["physical_gate_passed"])
            self.assertFalse(report["energy_gate_passed"])
            self.assertAlmostEqual(report["maximum_energy_rise_including_checkpoint_boundary_j"], .1)
            result["requested_steps"] = 3
            report = run()
            self.assertFalse(report["full_tail_complete"])
            self.assertFalse(report["quiet_diagnostic_passed"])
            result["requested_steps"] = 2
            result["frames"][0]["converged"] = False
            report = run()
            self.assertEqual(report["accepted_prefix_steps"], 0)
            self.assertEqual(report["samples"], [])
            self.assertFalse(report["physical_gate_passed"])
            self.assertFalse(report["energy_gate_passed"])
            result["frames"][0]["converged"] = True
            result["frames"][0]["energy_j"] = float("nan")
            report = run()
            self.assertFalse(report["energy_gate_passed"])
            self.assertFalse(report["quiet_diagnostic_passed"])
            self.assertIsNone(report["maximum_energy_rise_including_checkpoint_boundary_j"])


if __name__ == "__main__":
    unittest.main()
