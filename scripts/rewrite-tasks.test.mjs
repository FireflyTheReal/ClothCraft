import test from 'node:test';
import assert from 'node:assert/strict';
import { validateCatalog, issueBody, renderCatalog } from './rewrite-tasks.mjs';
function fixture() {
  return { schema_version: 1, source: { repository: 'example/project', commit: 'a'.repeat(40) }, tasks: [
    { id: 'EXO-CAD-001', title: 'Checked geometry', area: 'CAD', status: 'done', depends_on: [], scope: ['src/cad.rs'], acceptance: ['Independent geometry oracle passes'], evidence: ['https://example.com/spec'], related_prs: [], notes: 'Original implementation.' },
    { id: 'EXO-CAD-002', title: 'Editor command', area: 'CAD', status: 'ready', depends_on: ['EXO-CAD-001'], scope: ['src/editor/'], acceptance: ['Undo restores state'], evidence: ['https://example.com/spec'], related_prs: [], notes: '' }
  ] };
}
test('accepts a dependency-complete catalog and deterministic issue briefs', () => {
  const data = fixture(); assert.deepEqual(validateCatalog(data), []);
  assert.match(issueBody(data.tasks[1]), /Undo restores state/);
  assert.match(renderCatalog(data, { 'EXO-CAD-002': 'https://github.com/example/project/issues/1' }), /\[Issue\]/);
});
test('rejects duplicate IDs, missing references and cycles', () => {
  const duplicate = fixture(); duplicate.tasks.push(duplicate.tasks[0]);
  assert.ok(validateCatalog(duplicate).some(e => e.includes('Duplicate')));
  const missing = fixture(); missing.tasks[1].depends_on = ['EXO-CAD-999'];
  assert.ok(validateCatalog(missing).some(e => e.includes('unknown dependency')));
  const cycle = fixture(); cycle.tasks[0].depends_on = ['EXO-CAD-002'];
  assert.ok(validateCatalog(cycle).some(e => e.includes('cycle')));
});
test('blocks premature ready/done and review without a PR', () => {
  const data = fixture(); data.tasks[0].status = 'blocked';
  assert.ok(validateCatalog(data).some(e => e.includes('unfinished dependency')));
  data.tasks[1].status = 'review_existing';
  assert.ok(validateCatalog(data).some(e => e.includes('needs an existing PR')));
});
test('rejects unsafe scope paths and unpinned provenance', () => {
  for (const scope of ['../src', '/tmp/src', 'C:/repo', 'src\\cad.rs', 'src/**', 'src/./x']) {
    const data = fixture(); data.tasks[0].scope = [scope];
    assert.ok(validateCatalog(data).some(e => e.includes('unsafe scope')), scope);
  }
  const data = fixture(); data.source.commit = 'main';
  assert.ok(validateCatalog(data).some(e => e.includes('immutable SHA')));
});
test('malformed catalog fields return diagnostics rather than crashing', () => {
  assert.ok(validateCatalog(null).length);
  const data = fixture(); data.tasks.push(null);
  data.tasks[0].scope = {}; data.tasks[0].evidence = {};
  data.tasks[1].depends_on = 42;
  assert.ok(validateCatalog(data).length >= 4);
});
