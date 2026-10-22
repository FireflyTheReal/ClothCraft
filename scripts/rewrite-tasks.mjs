// Offline, dependency-free validation and rendering of the Rust rewrite backlog.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

export function validateCatalog(catalog) {
  const errors = [];
  if (catalog?.schema_version !== 1) errors.push('schema_version must be 1');
  if (!/^[\w.-]+\/[\w.-]+$/.test(catalog?.source?.repository ?? '')) errors.push('source.repository must be owner/repo');
  if (!/^[a-f0-9]{40}$/.test(catalog?.source?.commit ?? '')) errors.push('source.commit must be a full immutable SHA');
  if (!Array.isArray(catalog?.tasks) || !catalog.tasks.length) return [...errors, 'tasks must be a nonempty array'];
  const ids = new Set();
  const tasks = new Map();
  for (const task of catalog.tasks) {
    if (!task || typeof task !== 'object' || Array.isArray(task)) { errors.push('Each task must be an object'); continue; }
    const id = task?.id;
    if (!/^EXO-[A-Z]+-\d{3}$/.test(id ?? '')) errors.push(`Invalid task ID: ${id}`);
    if (ids.has(id)) errors.push(`Duplicate ID: ${id}`);
    ids.add(id); tasks.set(id, task);
    for (const key of ['title', 'area']) if (typeof task[key] !== 'string' || !task[key].trim()) errors.push(`${id}: missing ${key}`);
    if (!['ready', 'blocked', 'review_existing', 'done'].includes(task.status)) errors.push(`${id}: invalid status`);
    for (const key of ['scope', 'acceptance', 'evidence', 'related_prs', 'depends_on']) {
      if (!Array.isArray(task[key]) || task[key].some(item => typeof item !== 'string' || !item.trim())) errors.push(`${id}: invalid ${key}`);
    }
    if (!task.scope?.length || !task.acceptance?.length || !task.evidence?.length) errors.push(`${id}: scope, acceptance and evidence must not be empty`);
    for (const path of Array.isArray(task.scope) ? task.scope : []) {
      if (typeof path !== 'string' || /[\\*?\s]/.test(path) || path.startsWith('/') || path.includes(':') || path.split('/').some(p => p === '..' || p === '.') || path.includes('//')) errors.push(`${id}: unsafe scope path ${path}`);
    }
    for (const url of [...(Array.isArray(task.evidence) ? task.evidence : []), ...(Array.isArray(task.related_prs) ? task.related_prs : [])]) {
      if (typeof url !== 'string' || !/^https:\/\//.test(url)) errors.push(`${id}: evidence must use HTTPS`);
    }
    if (task.status === 'review_existing' && !task.related_prs?.length) errors.push(`${id}: review_existing needs an existing PR`);
  }
  const visited = new Set(), visiting = new Set();
  function visit(id) {
    if (visiting.has(id)) { errors.push(`Dependency cycle at ${id}`); return; }
    if (visited.has(id)) return;
    visiting.add(id);
    const task = tasks.get(id);
    for (const dep of Array.isArray(task?.depends_on) ? task.depends_on : []) {
      if (!tasks.has(dep)) errors.push(`${id}: unknown dependency ${dep}`);
      else {
        if (['ready', 'done'].includes(task.status) && tasks.get(dep).status !== 'done') errors.push(`${id}: ${task.status} with unfinished dependency ${dep}`);
        visit(dep);
      }
    }
    visiting.delete(id); visited.add(id);
  }
  for (const id of ids) visit(id);
  return errors;
}

export function issueBody(task) {
  return `Canonical task: **${task.id}**\n\nRead [the coordination guide](https://github.com/erythrofr/ClothCraft/blob/main/COORDINATION.md) and [the live board](https://github.com/erythrofr/ClothCraft/issues/8) before claiming. This issue's prose is a brief; the trusted manifest and coordinator acknowledgement determine claim eligibility.\n\n## Outcome\n\n${task.title}\n\n## Scope\n\n${task.scope.map(s => '- `' + s + '`').join('\n')}\n\nShared file changes require an expanded, reviewed scope before work starts.\n\n## Acceptance\n\n${task.acceptance.map(s => '- [ ] ' + s).join('\n')}\n\n## Dependencies and existing work\n\nManifest status: \`${task.status}\`. Dependencies: ${task.depends_on.length ? task.depends_on.join(', ') : 'none'}.\n\n${task.related_prs.length ? task.related_prs.map(s => '- ' + s).join('\n') : 'No existing PR recorded.'}\n\n## Evidence\n\n${task.evidence.map(s => '- ' + s).join('\n')}\n\n${Array.isArray(task.notes) ? task.notes.join('\n') : task.notes ?? ''}\n\nReport commands actually run and known limits. Preserve authoring compatibility, collision coverage and upstream attribution. This is a Rust implementation/review task, not permission to copy incompatible code or assets.\n`;
}

function cell(value) { return String(value).replaceAll('|', '\\|').replaceAll('\n', ' '); }
export function renderCatalog(catalog, issues = {}) {
  const lines = ['# Rust rewrite task catalog', '', 'Generated from `coordination/tasks.json` with `node scripts/rewrite-tasks.mjs markdown`. Do not edit this view directly.', '', '[Start and claim work](../COORDINATION.md) · [Live assignments](https://github.com/erythrofr/ClothCraft/issues/8) · [Feature audit](../docs/exo-feature-audit.md)', '', 'Manifest status describes eligibility, not current ownership. A ready task may already be claimed: check the live board and wait for acknowledgement. Blocked tasks are planning briefs; review_existing means review the linked contribution before implementing anything.', '', `Reference: [${catalog.source.repository}@${catalog.source.commit.slice(0, 12)}](https://github.com/${catalog.source.repository}/tree/${catalog.source.commit}). Reading evidence is not runtime validation.`, '', '| Task | Area | Status | Dependencies | Discussion |', '|---|---|---|---|---|'];
  for (const task of catalog.tasks) lines.push(`| [${task.id}](#${task.id.toLowerCase()}) — ${cell(task.title)} | ${cell(task.area)} | ${task.status} | ${task.depends_on.join(', ') || '—'} | ${issues[task.id] ? '[Issue](' + issues[task.id] + ')' : 'Not registered'} |`);
  for (const task of catalog.tasks) lines.push('', `## ${task.id}`, '', `**${task.title}**`, '', `Status: ${task.status}. Dependencies: ${task.depends_on.join(', ') || 'none'}.`, '', '**Files / directories**', '', ...task.scope.map(s => '- `' + s + '`'), '', '**Acceptance**', '', ...task.acceptance.map(s => '- ' + s), '', '**Evidence and related contributions**', '', ...[...task.evidence, ...task.related_prs].map(s => '- ' + s), '', ...(Array.isArray(task.notes) ? task.notes : [task.notes ?? '']));
  return lines.join('\n') + '\n';
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const catalog = JSON.parse(readFileSync(resolve(root, 'coordination/tasks.json'), 'utf8'));
  const errors = validateCatalog(catalog);
  if (errors.length) { console.error(errors.join('\n')); process.exitCode = 1; }
  else {
    const [command = 'check', id] = process.argv.slice(2);
    if (command === 'check') console.log(`Validated ${catalog.tasks.length} tasks, scopes and dependency graph.`);
    else if (command === 'markdown') {
      let issues = {};
      try { issues = JSON.parse(readFileSync(resolve(root, 'coordination/issues.json'), 'utf8')); }
      catch (e) { if (e.code !== 'ENOENT') throw e; }
      process.stdout.write(renderCatalog(catalog, issues));
    } else if (command === 'issue') {
      const task = catalog.tasks.find(t => t.id === id);
      if (!task) throw new Error(`Unknown task ${id}`);
      process.stdout.write(issueBody(task));
    } else if (command === 'list') {
      console.log(JSON.stringify(catalog.tasks.filter(t => !id || t.status === id), null, 2));
    } else throw new Error('Usage: rewrite-tasks.mjs check|markdown|issue TASK-ID|list [STATUS]');
  }
}
