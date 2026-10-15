# Contributing to ClothCraft

Humans and AI agents are welcome. Start with [AI_START_HERE.md](AI_START_HERE.md), [AGENTS.md](AGENTS.md), the [handoff](docs/handoff.md) and the [roadmap](ROADMAP.md). Both human and AI contributions follow the same physical contracts and verification requirements.

## Choose a small, verifiable change

The project is a very early alpha. Pick one bounded task with observable acceptance cases. Reliable save/open (`SAVE-01`) and partial-edge sewing (`SEW-01`) are useful first contributions. The [manual study](docs/manual-study.md) distinguishes public behavioral references, independently implemented features and work that remains planned. Do not describe roadmap formats or solver backends as available.

For physics changes, describe the affected energy/contact model, units, assumptions and limits. Add a test that could falsify the implementation: derivatives, objectivity, balance, rest-state preservation, an analytical result or convergence. Use the same scenes and settings when comparing solvers; report quality and full-step cost using the [solver challenge](docs/solver-challenge.md). Documentation and reversible visual changes do not need tests that merely repeat the implementation.

## Validate without a GPU

```powershell
.\scripts\check-cpu.ps1
```

The CPU workflow in [.github/workflows/cpu.yml](.github/workflows/cpu.yml) runs these checks and a small headless assembly. Inspect its actual result in GitHub Actions. For desktop type changes, `cargo build --locked -j 2` compiles without launching the interface. Keep local checks to two CPU jobs/test threads.

GPU tests and window checks are opt-in. Coordinate their use with the machine's owner; never add GPU work to the default test suite.

## Prepare the contribution

- Preserve `Cargo.lock` and existing document compatibility. Keep examples small, deterministic and explicit about uncalibrated material values.
- Preserve stable panel/boundary IDs and material-space rest geometry. Rebuild dependent caches coherently after physical edits; never infer rest shape from a drape.
- Keep `target/`, `output/`, heavy data and simulation caches out of Git. Small evidence belongs in `docs/`, with reproduction commands and limitations.
- Include the concrete before/after behavior, relevant validation and any remaining failures in the PR. Update the handoff and module documentation when behavior changes.
- Reference sources and distinguish reading a paper/manual, independent implementation and reused code. Do not include proprietary code, icons, example garments or other unauthorized assets.

## License and provenance

ClothCraft's original source and documentation are under the [MIT License](LICENSE), chosen by the project owner. Contributions intended for inclusion must be compatible with that license; retain any applicable third-party notices. The bundled MakeHuman assets retain their [CC0 notice](assets/makehuman/LICENSE.ASSETS.md), and dependencies retain their own licenses. The [artwork record](assets/branding/README.md) documents the generated icon and its references.

## Coordinate across forks

A fork is an independent working copy, not an automatic subscription or a promise to contribute. Keep it synchronized with upstream before starting a change. Announce a small scope in an issue, draft PR or the community Discord when possible; a short message prevents overlapping edits without requiring meetings.

Open one focused PR per feature against `erythrofr/ClothCraft:main`, explain the behavior and tests, and preserve unrelated work. Discuss shared model changes before rewriting authoring, sewing or document schemas. If you import work from another fork, preserve the author and source commit references; credit adapted code and document integration fixes. Do not imply endorsement by that contributor. No Discord message or GitHub mention is sent automatically by these guidelines.

The first selective integration from FireflyTheReal / julienfoss is recorded in [the integration notes](docs/firefly-integration.md).
