# Contributor kit

ClothCraft needs help turning a physics prototype into a usable garment tool. Pick a small piece of the work and make its result easy for someone else to check. You can contribute observations and hypotheses without writing code.

## Choose a track

| Track | Useful first contribution | Evidence |
|---|---|---|
| Editing and interface | Observe someone drafting and sewing a simple top; fix one confusing interaction | Exact before/after action sequence, cancellation and undo behavior |
| Solver diagnosis | Isolate one oscillation, stall or convergence failure | Small fixture, settings, state history and force/contact diagnostics |
| Performance | Profile one measured full-step bottleneck | Matched quality checks, hardware/revision and complete timing distribution |
| Materials | Find a reusable measured directional dataset | Source, rights, units, test protocol and a proposed constitutive fit |
| Interchange | Implement one direction of one format | Supported entities, independent fixture, units/axes and loss report |
| Documentation and testing | Reproduce a setup or identify a misleading claim | Exact commands, actual output and a focused correction |

Current task IDs and acceptance criteria live in [the roadmap](../ROADMAP.md#current-contribution-queue). Discuss overlapping changes through an issue or draft PR. A fork does not synchronize automatically with upstream.

## Report a problem

Copy and complete this outline in an issue:

```text
Revision / operating system / CPU / GPU:
What I tried to do:
Steps to reproduce:
Expected behavior:
Observed behavior:
Mesh spacing and solver settings:
Does it happen from a fresh launch or only after an edit / grab / reload?
Small original garment or diagnostic report (if shareable):
Recording or screenshot:
```

For performance, include physical steps per second, dt and wall time. Viewport FPS alone cannot tell us whether cloth runs in real time. For settling, say how long the garment was simulated before the capture and whether it was still assembling. Do not zero velocities to make a case look settled.

For an idea rather than a bug, explain the mechanism you suspect, which existing evidence supports it and the smallest experiment that could disprove it. An untested suggestion is useful when clearly labelled.

## Give a coding assistant a bounded task

Start any task with this preamble:

> Read AGENTS.md, AI_START_HERE.md, the current handoff and the documentation of the affected module. Inspect the existing implementation and preserve unrelated changes. Select one task below. Explain its scope and acceptance cases, implement it, run the relevant checks and report the actual results and remaining limits. Keep all new UI text, comments and documentation in English. Do not activate experimental solver behavior, use the GPU or publish changes without authorization.

### Interface task

> Take one open UX task from ROADMAP.md. Reproduce the confusing interaction before editing. Keep geometry operations transactional and undoable, with shared semantic selections in 2D/3D. Test cancel, undo/redo, keyboard input in text fields and save/reload. Do not alter the physical model to fix an interface problem. Show a short action sequence that an unfamiliar user can follow.

### Solver investigation

> Take PHYS-01 or PHYS-02. Preserve the production baseline and construct a small independent reproduction. Separate changes caused by contact refresh, solver residuals and time discretization. Evaluate force balance and energy alongside motion. Test rest, sliding and pull/hold/release; a frozen or sticky garment is not a pass. Keep a candidate opt-in until all gates pass. Record failures and do not tailor the algorithm to private garment IDs.

### Performance task

> Take PERF-01. Profile the complete physical step before optimizing. Keep the mesh, dt, iteration budget, materials, seam map and collision coverage fixed. Measure transfers, waits, allocations, contact search and solve separately. Compare complete-step P50/P95 and simulated/wall time on the same fixture, with a scalar or reference correctness check. Do not report a kernel timestamp as end-to-end speed.

### Format task

> Take one IO task and one direction: import or export. Read docs/interoperability.md. Specify units, axes, supported entities, topology policy and unsupported-data diagnostics first. Keep the adapter outside solver code. Use self-authored or appropriately licensed fixtures and verify the result with an independent reader or writer. Passing one sample does not establish full format support.

## A contribution is ready when

- Its scope and user-visible behavior are clear, and unrelated work is preserved.
- Relevant checks actually pass; failed or unavailable checks are disclosed.
- Physics changes include a falsifiable quality test, not only a visually quieter result.
- New data has provenance; private garments, output captures, binaries and caches stay out of Git.
- Documentation explains supported behavior and remaining limitations without presenting research candidates as production backends.
- The PR gives another contributor the commands and small fixture needed to reproduce the result.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for CPU checks, licensing and fork coordination. GPU checks remain opt-in. No research or performance claim is accepted solely because an AI assistant produced it.
