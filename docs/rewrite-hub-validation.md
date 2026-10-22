# Rust rewrite hub validation

22 October 2026. The feature audit pins EXO main at
`352c6086b622caeda95f16d9bd923624b95e05d9`. It is source inspection, not a
runtime certification of EXO's tools. No EXO implementation or assets were
copied. Three delegated agents handled the inventory, coordinator and initial
Rust quantities slice, followed by independent coordinator review.

## Local checks

- CPU suite: **288 passed**, with two build jobs and two test threads.
- CPU all-target Clippy with warnings denied: passed.
- Rust formatting and Git whitespace checks: passed.
- Coordinator: **30 Node tests passed**. Coverage includes competing claims,
  overlapping scopes, author/session identity, explicit release, stale locks,
  dependencies, replay, invalid commands, bot spoofing, API failures, durable
  write ordering and deleted comments shifting a pagination boundary.
- Catalog: **5 Node tests passed**, plus validation of all 67 task IDs,
  immutable upstream reference, scopes and the dependency graph.
- Local links in the entry point, audit, protocol and generated catalog were
  checked; no missing targets were found.
- Net material metrics: five CPU tests included in the total above. The public
  T-shirt CLI report gives **0.7976 m2** and **0.15952 kg**, grouped by panel and
  stable fabric ID. These are net cloth quantities, not purchasing estimates.

No local GPU workload or native window was used. No user garment was used as
a benchmark or modified; its saved file hash remains unchanged. The solver,
physical laws and collision coverage were not changed.
Local full test output is in `output/rewrite-hub-20261022/cpu-tests.log`,
excluded from Git.

## Hosted validation

The [initial coordinator run](https://github.com/erythrofr/ClothCraft/actions/runs/39828252877)
successfully registered all 16 initial task issues and created the durable
bot-owned ledger. The [catalog CI run](https://github.com/erythrofr/ClothCraft/actions/runs/39828252906)
passed. The [claim run](https://github.com/erythrofr/ClothCraft/actions/runs/39828397564)
accepted `erythrofr/jordane-codex-smoke` on the dedicated
[protocol test issue](https://github.com/erythrofr/ClothCraft/issues/9).

The live ledger then rejected `competing-smoke-session`, accepted the owner's
heartbeat, entered review while retaining ownership, and accepted the explicit
release. The [release workflow](https://github.com/erythrofr/ClothCraft/actions/runs/39828636229)
succeeded and the ledger was observed with **zero active claims**. The rapid
command sequence included a canceled pending run
(`39828501482`); subsequent reconciliation still processed all commands.
Receipts are preserved on that issue and the
[authoritative board](https://github.com/erythrofr/ClothCraft/issues/8).
This checks GitHub permissions and webhook processing as well as the offline
reducer. It is not a guarantee of GitHub availability or a distributed agent
runtime: agents must wait for acknowledged ownership and arrange their own
authorized session startup or polling.

## Compiler compatibility

The previous published commit's CPU tests passed on GitHub, but Clippy 1.99.1
introduced a denied `chunks_exact_to_as_chunks` lint. Local Rust was 1.97.1.
The fixed-size test/example loops now use `as_chunks::<3>().0.iter()`, retaining
the same ordering and ignored remainder. Dynamic chunk sizes and production
solver source are unchanged. The `tests/drafting_tools.rs` fix matches
[FireflyTheReal PR #4](https://github.com/erythrofr/ClothCraft/pull/4), commit
`43018fc959b311a6029bb0ffc586619786938e50`; the same compatibility change covers
the newer research fixtures. The PR remains open for maintainer reconciliation.

## Remaining limits

Claims are cooperative reservations, not branch protection. The owner and
trusted workflows can change the ledger. Scope changes need review; a stale
claim is not automatically reassigned. History is bounded at 10,000 comments
and state at 60,000 characters, with failure instead of partial processing.
Cross-account release rejection is tested offline; the live lifecycle uses
one authorized account with two session labels. The pending EXO-related PRs
are not merged or declared validated by creating the task inventory.
