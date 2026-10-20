"""CPU-only two-degree-of-freedom research model, NOT a cloth backend.

Two equal masses, outer elastic supports and an incompatible seam rest length.
Compare 16 exact scalar Gauss-Seidel sweeps with a coupled 2x2 backward-Euler
solve of the SAME quadratic potential. No contact and no damping adjustment.
"""
import argparse
import json
from pathlib import Path

MASS, DT, DRAG = 0.001, 1.0 / 60.0, 1.5
SUPPORT, SEAM, SPAN, REST = 1.0, 1000.0, 0.1, 0.05


def solve2(a, b, d, rhs):
    det = a * d - b * b
    return [(d * rhs[0] - b * rhs[1]) / det,
            (a * rhs[1] - b * rhs[0]) / det]


def potential(x, target):
    value = 0.5 * SUPPORT * (x[0] ** 2 + (x[1] - SPAN) ** 2)
    value += 0.5 * SEAM * (x[1] - x[0] - REST) ** 2
    return value + (0.5 * 10.0 * (x[1] - target) ** 2 if target is not None else 0.0)


def run(coupled):
    eq = solve2(SUPPORT + SEAM, -SEAM, SUPPORT + SEAM,
                [-SEAM * REST, SUPPORT * SPAN + SEAM * REST])
    x, v, old_target = eq.copy(), [0.0, 0.0], None
    rows = []
    for step in range(360):
        phase = 'rest' if step < 60 else 'pull' if step < 120 else 'hold' if step < 150 else 'release'
        t = min(1.0, max(0.0, (step - 59) / 60.0))
        target = eq[1] + 0.02 * t * t * (3 - 2 * t) if 60 <= step < 150 else None
        kg = 10.0 if target is not None else 0.0
        k00, k01, k11 = SUPPORT + SEAM, -SEAM, SUPPORT + SEAM + kg
        load = [-SEAM * REST, SUPPORT * SPAN + SEAM * REST + kg * (target or 0.0)]
        inertia = MASS * (1 + DRAG * DT) / DT ** 2
        y = [x[i] + DT * v[i] / (1 + DRAG * DT) for i in range(2)]
        rhs = [inertia * y[i] + load[i] for i in range(2)]
        old_x, old_v = x.copy(), v.copy()
        if coupled:
            x = solve2(inertia + k00, k01, inertia + k11, rhs)
        else:
            x = y.copy()
            for _ in range(16):
                x[0] = (rhs[0] - k01 * x[1]) / (inertia + k00)
                x[1] = (rhs[1] - k01 * x[0]) / (inertia + k11)
        dx = [x[i] - old_x[i] for i in range(2)]
        v = [d / DT for d in dx]
        dv = [v[i] - old_v[i] for i in range(2)]
        grad = [k00 * x[0] + k01 * x[1] - load[0], k01 * x[0] + k11 * x[1] - load[1]]
        residual = [MASS * dv[i] / DT + DRAG * MASS * v[i] + grad[i] for i in range(2)]
        old_e = potential(old_x, target) + 0.5 * MASS * sum(q*q for q in old_v)
        new_e = potential(x, target) + 0.5 * MASS * sum(q*q for q in v)
        drag = DT * DRAG * MASS * sum(q*q for q in v)
        implicit = 0.5 * MASS * sum(q*q for q in dv)
        remainder = -0.5 * (k00*dx[0]**2 + 2*k01*dx[0]*dx[1] + k11*dx[1]**2)
        residual_work = sum(residual[i]*dx[i] for i in range(2))
        error = new_e-old_e + drag + implicit - remainder - residual_work
        assert abs(error) < 1e-14
        if coupled:
            assert new_e-old_e <= 1e-14, 'Autonomous exact quadratic BE must dissipate'
        if step < 60:
            assert max(abs(x[i]-eq[i]) for i in range(2)) < 1e-12
        rows.append(dict(step=step, phase=phase, x=x.copy(), v=v.copy(),
                         total_j=new_e, autonomous_delta_j=new_e-old_e,
                         control_change_j=potential(old_x,target)-potential(old_x,old_target),
                         residual_work_j=residual_work, drag_loss_j=drag,
                         implicit_loss_j=implicit, nonlinear_remainder_j=remainder,
                         balance_error_j=error))
        old_target = target
    release = rows[150:]
    return rows, dict(equilibrium=eq, stored_equilibrium_energy_j=potential(eq,None),
                      max_release_energy_rise_j=max(r['autonomous_delta_j'] for r in release),
                      release_energy_rise_steps=sum(r['autonomous_delta_j'] > 1e-14 for r in release),
                      max_release_residual_work_j=max(r['residual_work_j'] for r in release),
                      final_offset_m=max(abs(x[i]-eq[i]) for i in range(2)),
                      maximum_balance_error_j=max(abs(r['balance_error_j']) for r in rows))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=Path('output/energy-research/seam-model'))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    summary = dict(model='two_scalar_masses_no_contact', gpu_used=False,
                   parameters=dict(mass=MASS, dt=DT, drag=DRAG, support=SUPPORT, seam=SEAM, span=SPAN, rest=REST))
    for label, coupled in [('scalar_16', False), ('coupled_exact', True)]:
        rows, summary[label] = run(coupled)
        (args.output / (label+'.json')).write_text(json.dumps(rows,indent=2)+'\n')
    (args.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))
