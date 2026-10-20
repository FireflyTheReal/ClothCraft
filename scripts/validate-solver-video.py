"""Validate recorded frame correspondence; does not replace collision audits."""
import argparse
import hashlib
import json
import math
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path, required=True)
    args = parser.parse_args()
    root = args.input
    provenance = json.loads((root/'provenance.json').read_text())
    reference = None
    reference_geometry = None
    records = []
    for run in provenance['runs']:
        folder = root/run['backend']
        initial = json.loads((folder/'initial-state.json').read_text())
        geometry = json.loads((folder/'render-geometry.json').read_text())
        protocol = json.loads((folder/'protocol.json').read_text())
        final = json.loads((folder/'final-state.json').read_text())
        metrics = [json.loads(s) for s in (folder/'steps.jsonl').open()]
        summary = json.loads((folder/'summary.json').read_text())
        if reference is None:
            reference, reference_geometry = initial, geometry
        assert initial == reference and geometry == reference_geometry
        count = 0
        for index, line in enumerate((folder/'positions.jsonl').open()):
            frame = json.loads(line)
            assert frame['step'] == index == metrics[index]['step']
            assert len(frame['x']) == len(initial['x'])
            assert all(math.isfinite(v) for p in frame['x'] for v in p)
            assert abs(frame['time']-initial['time']-(index+1)*protocol['settings']['dt']) < 1e-10
            count += 1
        assert count == len(metrics) == summary['accepted_steps'] and count > 0
        assert frame['x'] == final['x'] and frame['time'] == final['time']
        collision_failure = summary['failure'] and 'Surface intersection after accepted step' in summary['failure']
        records.append({'backend': run['backend'], 'recorded_steps': count,
                        'separation_checked_prefix_steps': count-1 if collision_failure else count,
                        'completed_all_requested_steps': count == provenance['steps'] and summary['failure'] is None,
                        'failure': summary['failure'], 'final_intersection_check': summary['final_intersection_check'],
                        'mean_cpu_ms': summary['mean_ms'], 'p95_cpu_ms': summary['p95_ms'],
                        'final_seam_gap_mm': 1000*metrics[-1]['metrics']['max_seam_gap_m'],
                        'final_stretch_p95_percent': metrics[-1]['metrics']['extension_p95_percent']})
    original = Path(provenance['original_project'])
    assert hashlib.sha256(original.read_bytes()).hexdigest() == provenance['original_sha256']
    report = {'same_initial_state_and_geometry': True, 'frame_time_correspondence': True,
              'finite_positions': True, 'original_save_unchanged': True,
              'scope': 'Five CPU implementations. No solver modifications. Recorded endpoint checks are not an exact continuous certificate.',
              'runs': records}
    (root/'video-validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
