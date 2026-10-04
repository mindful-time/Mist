import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { env as environment } from 'node:process';
import { test } from 'node:test';

const root = path.resolve(import.meta.dirname, '..');
const repository = 'mindful-time/Mist';
const commit = 'a'.repeat(40);
const prefix = `/repos/${repository}/actions`;
const required = [
  'Repository metadata, workflows, and website',
  'Quality and security gates',
  'check (macos-latest)',
  'check (windows-latest)',
  'check (ubuntu-latest)',
  'Linux package validation',
];

function fixture(t) {
  const directory = mkdtempSync(path.join(tmpdir(), 'mist-release-ci-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const run = {
    id: 123, workflow_id: 456, run_attempt: 1,
    event: 'push', head_branch: 'main', head_sha: commit,
    status: 'completed', conclusion: 'success', path: '.github/workflows/ci.yml',
    repository: { full_name: repository }, head_repository: { full_name: repository },
  };
  const responses = {
    [`${prefix}/workflows/ci.yml`]: { id: 456, state: 'active', path: '.github/workflows/ci.yml' },
    [`${prefix}/workflows/456/runs?page=1`]: { total_count: 1, workflow_runs: [run] },
    [`${prefix}/runs/123`]: run,
    [`${prefix}/runs/123/attempts/1/jobs?page=1`]: {
      total_count: 6,
      jobs: required.map(name => ({ name, head_sha: commit, run_id: 123, run_attempt: 1, status: 'completed', conclusion: 'success' })),
    },
  };
  const updates = {};
  return {
    responses, run, updates,
    verify(overrides = {}) {
      const preload = path.join(directory, 'github-api.mjs');
      // Mock only GitHub's external HTTP boundary, not the verification code.
      writeFileSync(preload, `const responses = ${JSON.stringify(responses)};
const updates = ${JSON.stringify(updates)};
globalThis.fetch = async input => {
  const url = new URL(input);
  if (url.origin !== 'https://api.github.com') throw new Error('Unexpected API host');
  const key = url.pathname + (url.searchParams.has('page') ? '?page=' + url.searchParams.get('page') : '');
  if (!(key in responses)) return { ok: false, status: 404 };
  const value = responses[key];
  if (key in updates) responses[key] = updates[key];
  return { ok: true, json: async () => value };
};\n`);
      return spawnSync(process.execPath, ['--import', preload, 'scripts/verify-release-ci.mjs'], {
        cwd: root, encoding: 'utf8',
        env: { ...environment, GITHUB_REPOSITORY: repository, GITHUB_SHA: commit, GH_TOKEN: 'fixture-token', ...overrides },
      });
    },
  };
}

test('release does not fall back to old successful CI after a newer run fails', t => {
  const f = fixture(t);
  const newer = { ...f.run, id: 124, conclusion: 'failure' };
  f.responses[`${prefix}/workflows/456/runs?page=1`] = { total_count: 2, workflow_runs: [f.run, newer] };
  f.responses[`${prefix}/runs/124`] = newer;
  assert.notEqual(f.verify().status, 0);
});

test('release rejects a CI rerun that starts while its job evidence is being checked', t => {
  const f = fixture(t);
  f.updates[`${prefix}/runs/123`] = { ...f.run, run_attempt: 2, status: 'in_progress', conclusion: null };
  assert.notEqual(f.verify().status, 0);
});

test('release checks required jobs on every API page', t => {
  const f = fixture(t);
  const key = `${prefix}/runs/123/attempts/1/jobs`;
  const jobs = f.responses[`${key}?page=1`].jobs;
  f.responses[`${key}?page=1`] = { total_count: 6, jobs: jobs.slice(0, 3) };
  f.responses[`${key}?page=2`] = { total_count: 6, jobs: jobs.slice(3) };
  const result = f.verify();
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test('release rejects a newer failed run on a later API page', t => {
  const f = fixture(t);
  f.responses[`${prefix}/workflows/456/runs?page=1`].total_count = 2;
  const newer = { ...f.run, id: 124, conclusion: 'failure' };
  f.responses[`${prefix}/workflows/456/runs?page=2`] = { total_count: 2, workflow_runs: [newer] };
  f.responses[`${prefix}/runs/124`] = newer;
  assert.notEqual(f.verify().status, 0);
});

test('release fails closed when GitHub job evidence is unavailable or incomplete', t => {
  const f = fixture(t);
  const key = `${prefix}/runs/123/attempts/1/jobs?page=1`;
  const page = f.responses[key];
  delete f.responses[key];
  assert.notEqual(f.verify().status, 0);
  f.responses[key] = { ...page, total_count: 7 };
  assert.notEqual(f.verify().status, 0);
});

test('release requires its GitHub token rather than silently using anonymous evidence', t => {
  const f = fixture(t);
  assert.notEqual(f.verify({ GH_TOKEN: '' }).status, 0);
});

test('release accepts a successful main CI run with all required checks for its exact commit', t => {
  const f = fixture(t);
  const result = f.verify();
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /Verified CI/);
});

for (const [name, changes] of [
  ['a different commit', { head_sha: 'b'.repeat(40) }],
  ['a PR run rather than main integration CI', { event: 'pull_request' }],
  ['a different branch', { head_branch: 'topic' }],
  ['a different workflow', { workflow_id: 999 }],
  ['a different workflow path', { path: '.github/workflows/other.yml' }],
  ['a fork repository', { repository: { full_name: 'contributor/Mist' } }],
  ['a fork source', { head_repository: { full_name: 'contributor/Mist' } }],
  ['unfinished CI', { status: 'in_progress', conclusion: null }],
  ['failed CI', { conclusion: 'failure' }],
]) {
  test(`release rejects ${name} even when other CI evidence looks successful`, t => {
    const f = fixture(t);
    Object.assign(f.run, changes);
    assert.notEqual(f.verify().status, 0);
  });
}

test('release rejects a commit without a main CI run', t => {
  const f = fixture(t);
  f.responses[`${prefix}/workflows/456/runs?page=1`] = { total_count: 0, workflow_runs: [] };
  assert.notEqual(f.verify().status, 0);
});

for (const [name, change] of [
  ['a failed required check', jobs => { jobs[1].conclusion = 'failure'; }],
  ['a skipped required check', jobs => { jobs[1].conclusion = 'skipped'; }],
  ['an unfinished required check', jobs => { jobs[1].status = 'in_progress'; }],
  ['a missing required check', jobs => { jobs.pop(); }],
  ['a duplicate required check', jobs => { jobs.push({ ...jobs[1] }); }],
  ['a job from another commit', jobs => { jobs[1].head_sha = 'b'.repeat(40); }],
  ['a job from another run', jobs => { jobs[1].run_id = 999; }],
  ['a job from an old attempt', jobs => { jobs[1].run_attempt = 0; }],
]) {
  test(`release rejects ${name} even if the workflow conclusion is success`, t => {
    const f = fixture(t);
    const page = f.responses[`${prefix}/runs/123/attempts/1/jobs?page=1`];
    change(page.jobs);
    page.total_count = page.jobs.length;
    assert.notEqual(f.verify().status, 0);
  });
}
