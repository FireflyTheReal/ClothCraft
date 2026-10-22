import {readFile} from 'node:fs/promises';
import {
  STATE_MARKER, TASK_MARKER, emptyState, parseCommand, parseHub,
  reduceComments, renderHub, validateManifest,
} from './engine.mjs';

const BOT = 'github-actions[bot]';

// Deliberately paginate with a hard cap: do not advance the cursor over a
// truncated result. A larger project should migrate the event store explicitly.
export async function boundedList(fetchPage, maximum) {
  const values = [];
  for (let page = 1; ; page += 1) {
    const {data} = await fetchPage(page);
    if (!Array.isArray(data)) throw new Error('Expected a GitHub list');
    values.push(...data);
    if (values.length > maximum) throw new Error('Comment scan limit exceeded; state is unchanged');
    if (data.length < 100) return values;
  }
}

function commentFingerprint(comments) {
  // Include everything used to identify, authorize or parse a command. A
  // body edit must be detected even when the comment ID did not change.
  return JSON.stringify(comments.map((c) => [
    c.id, c.issue_url, c.body, c.user?.login, c.user?.type,
    c.created_at, c.updated_at,
  ]));
}

export async function stableComments(fetchPage, maximum) {
  // Page-number pagination is mutable. Deleting an older comment while we
  // advance pages can shift an unseen command behind the next page boundary.
  // Refuse to advance the durable cursor unless two complete scans agree.
  const first = await boundedList(fetchPage, maximum);
  const fingerprint = commentFingerprint(first);
  const second = await boundedList(fetchPage, maximum);
  if (fingerprint !== commentFingerprint(second)) {
    throw new Error('Comment history changed during reconciliation; retry without advancing state');
  }
  return second;
}

export async function reconcile({github, context, core}) {
  const config = JSON.parse(await readFile('scripts/coordination/config.json', 'utf8'));
  const manifest = JSON.parse(await readFile('coordination/tasks.json', 'utf8'));
  return reconcileSnapshot({github, context, core, config, manifest});
}

export async function reconcileSnapshot({github, context, core, config, manifest}) {
  validateManifest(manifest);
  const {owner, repo} = context.repo;
  if (config.repository !== `${owner}/${repo}` || !Number.isSafeInteger(config.hub_issue) ||
      config.hub_issue <= 0 || !Number.isSafeInteger(config.max_comments) ||
      config.max_comments < 100 || config.max_comments > 100000 ||
      !Number.isFinite(config.stale_hours) || config.stale_hours <= 0) {
    throw new Error('Coordinator is unconfigured or running in a noncanonical repository');
  }
  const base = {owner, repo};
  const hub = await github.rest.issues.get({...base, issue_number: config.hub_issue});
  if (hub.data.pull_request || hub.data.state !== 'open') throw new Error('Hub must be an open issue');
  const comments = await stableComments((page) => github.rest.issues.listCommentsForRepo({
    ...base, sort: 'created', direction: 'asc', per_page: 100, page,
  }), config.max_comments);
  const issueNumber = (comment) => {
    const prefix = `https://api.github.com/repos/${owner}/${repo}/issues/`;
    if (!comment.issue_url?.startsWith(prefix)) throw new Error('Unexpected comment repository');
    const number = Number(comment.issue_url.slice(prefix.length));
    if (!Number.isSafeInteger(number) || number <= 0) throw new Error('Invalid comment issue');
    return number;
  };
  const stateComments = comments.filter((c) => issueNumber(c) === config.hub_issue &&
    c.user?.login === BOT && c.user?.type === 'Bot' && c.body?.startsWith(STATE_MARKER));
  if (stateComments.length > 1) throw new Error('Multiple state comments; maintainer repair required');
  const canonical = stateComments[0];
  const previous = canonical ? parseHub(canonical.body) : emptyState();
  // Once a ledger exists, deletion must never silently bootstrap another one.
  // The hub body marker is written only after a durable state comment exists.
  const initialized = '<!-- clothcraft-coordination-initialized-v1 -->';
  if (!canonical && hub.data.body?.includes(initialized)) {
    throw new Error('Durable state was deleted. Restore it; do not reset active claims.');
  }
  const permissions = new Map();
  const issues = new Map([[config.hub_issue, hub.data]]);
  const events = [];
  for (const comment of comments.filter((c) => c.id > previous.after_comment)) {
    const number = issueNumber(comment);
    const actor = comment.user?.login;
    const bot = comment.user?.type === 'Bot';
    const command = !bot && parseCommand(comment.body);
    let permission = 'read';
    if (command) {
      if (!issues.has(number)) {
        const result = await github.rest.issues.get({...base, issue_number: number});
        issues.set(number, result.data);
      }
      if (['register', 'release-force'].includes(command.verb)) {
        if (!permissions.has(actor)) {
          try {
            const result = await github.rest.repos.getCollaboratorPermissionLevel({...base, username: actor});
            permissions.set(actor, result.data.permission);
          } catch (error) {
            if (error.status !== 404) throw error;
            permissions.set(actor, 'read');
          }
        }
        permission = permissions.get(actor);
      }
    }
    events.push({
      id: comment.id, issue: number, actor, bot, permission, body: comment.body,
      created_at: comment.created_at,
      edited: comment.updated_at !== comment.created_at,
      closed: issues.get(number)?.state === 'closed',
      pull_request: Boolean(issues.get(number)?.pull_request),
    });
  }
  const next = reduceComments(previous, manifest, events, config.hub_issue);
  const body = renderHub(next, manifest, config.repository, new Date().toISOString(), config.stale_hours);
  // Commit ownership before publishing any convenience projection/receipt.
  // If the update fails, the run fails and no task may treat its claim as accepted.
  if (canonical) {
    if (canonical.body !== body) {
      await github.rest.issues.updateComment({...base, comment_id: canonical.id, body});
    }
  } else {
    await github.rest.issues.createComment({...base, issue_number: config.hub_issue, body});
  }
  if (!hub.data.body?.includes(initialized)) {
    await github.rest.issues.update({...base, issue_number: config.hub_issue,
      body: `${hub.data.body ?? ''}\n\n${initialized}`});
  }
  // These comments are explicitly nonauthoritative. Losing a projection after
  // the state write is harmless; the next reconciliation repairs it.
  for (const [id, task] of Object.entries(next.tasks)) {
    const c = task.claim;
    const last = [...next.recent].reverse().find((r) => r.issue === task.issue);
    const display = [TASK_MARKER, `Coordination for **${id}**.`, '',
      c ? `Reserved by **${c.owner} / ${c.agent}** (${c.phase}).` : 'No active reservation.',
      last ? `Latest command: ${last.accepted ? 'accepted' : 'rejected'} — ${last.message}` : '',
      '', `**Always check the [authoritative board](https://github.com/${config.repository}/issues/${config.hub_issue}) before starting.**`,
      'This projection may lag after a failed workflow. Closing an issue or opening a PR does not release its reservation.',
    ].filter(Boolean).join('\n');
    const projections = comments.filter((item) => issueNumber(item) === task.issue &&
      item.user?.login === BOT && item.user?.type === 'Bot' && item.body?.startsWith(TASK_MARKER));
    if (projections.length > 1) throw new Error(`Duplicate task projections on issue ${task.issue}`);
    if (projections.length) {
      if (projections[0].body !== display) {
        await github.rest.issues.updateComment({...base, comment_id: projections[0].id, body: display});
      }
    } else {
      await github.rest.issues.createComment({...base, issue_number: task.issue, body: display});
    }
  }
  core.info(`Reconciled ${events.length} comments; ${Object.keys(next.tasks).length} registered tasks.`);
}
