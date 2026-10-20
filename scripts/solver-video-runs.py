"""Record matched CPU dress trajectories. No solver changes or GPU workload."""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--project', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--exe', type=Path, default=Path('target/release/examples/solver_compare.exe'))
    parser.add_argument('--steps', type=int, default=350)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    project = args.project.resolve()
    data = json.loads(project.read_text(encoding='utf-8'))
    avatar = (project.parent / data['avatar_path']).resolve()
    source = out / 'input' / 'source.json'
    source.parent.mkdir()
    shutil.copy2(avatar, source.parent / 'avatar.obj')
    data['avatar_path'] = 'avatar.obj'
    source.write_text(json.dumps(data, indent=2), encoding='utf-8')
    exe = out / 'solver_compare.exe'
    shutil.copy2(args.exe, exe)
    provenance = {'original_project': str(project), 'original_sha256': sha(project),
                  'executable_sha256': sha(exe), 'steps': args.steps, 'dt_s': 1/60,
                  'iterations': 4, 'threads': 2, 'gpu': False, 'runs': [],
                  'timing_scope': 'Accepted CPU step only; export/render/audits excluded. Background load uncontrolled.',
                  'source_hashes': {}}
    for folder in ('src', 'examples', 'tests'):
        for p in Path(folder).rglob('*'):
            if p.is_file():
                dst = out / 'source' / p
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(p, dst)
                provenance['source_hashes'][p.as_posix()] = sha(p)
    for p in map(Path, ['Cargo.toml', 'Cargo.lock', 'scripts/solver-video-runs.py']):
        dst = out / 'source' / p
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, dst)
        provenance['source_hashes'][p.as_posix()] = sha(p)
    for backend in ['vbd', 'xpbd', 'avbd-finite', 'xpbd-primal', 'avbd-target']:
        command = [str(exe), '--project', str(source), '--output', str(out / backend),
                   '--backend', backend, '--steps', str(args.steps), '--iterations', '4',
                   '--dt', str(1/60), '--pull', '--record-states', '--audit-every-step']
        (out / 'progress.json').write_text(json.dumps({'running': backend, 'completed': provenance['runs']}))
        print('Starting ' + backend, flush=True)
        with (out / (backend + '.stdout.log')).open('w') as stdout, (out / (backend + '.stderr.log')).open('w') as stderr:
            result = subprocess.run(command, stdout=stdout, stderr=stderr, check=False)
        summary_path = out / backend / 'summary.json'
        summary = json.loads(summary_path.read_text()) if summary_path.exists() else {'failure': 'No summary produced'}
        provenance['runs'].append({'backend': backend, 'command': command, 'exit_code': result.returncode, 'summary': summary})
        (out / 'provenance.json').write_text(json.dumps(provenance, indent=2), encoding='utf-8')
        print(f'Finished {backend}: exit={result.returncode}, steps={summary.get("accepted_steps")}', flush=True)
    assert sha(project) == provenance['original_sha256'], 'Original save changed during runs'
    (out / 'progress.json').write_text(json.dumps({'running': None, 'completed': provenance['runs']}))


if __name__ == '__main__':
    main()
