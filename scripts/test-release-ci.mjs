import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
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
  'check (macos-15-intel)',
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
    repository: { id: 42, full_name: repository }, head_repository: { id: 42, full_name: repository },
  };
  const responses = {
    [`${prefix}/workflows/ci.yml`]: { id: 456, state: 'active', path: '.github/workflows/ci.yml' },
    [`${prefix}/workflows/456/runs?page=1`]: { total_count: 1, workflow_runs: [run] },
    [`${prefix}/runs/123`]: run,
    [`${prefix}/runs/123/attempts/1/jobs?page=1`]: {
      total_count: 7,
      jobs: required.map(name => ({ name, head_sha: commit, run_id: 123, run_attempt: 1, status: 'completed', conclusion: 'success' })),
    },
    [`${prefix}/runs/123/artifacts?page=1`]: {
      total_count: 4,
      artifacts: ['linux', 'macos-aarch64', 'macos-x86_64', 'windows-x86_64'].map((platform, index) => ({
        id: 789 + index, name: `mist-ci-${platform}-${commit}-attempt-1`,
        size_in_bytes: 20, digest: 'sha256:03bbb0874ab942ba8e81bb2d49032b3737d834e5ee0e323f04718f233721e882',
        expired: false, expires_at: '2099-01-01T00:00:00Z',
        workflow_run: { id: 123, head_sha: commit, head_branch: 'main', repository_id: 42, head_repository_id: 42 },
      })),
    },
  };
  const updates = {};
  const storage = { body: 'ci-candidate-fixture', location: 'https://storage.example/candidate.zip', status: 302 };
  return {
    responses, run, updates, directory, storage,
    verify(overrides = {}) {
      const preload = path.join(directory, 'github-api.mjs');
      // Mock only GitHub's external HTTP boundary, not the verification code.
      writeFileSync(preload, `const responses = ${JSON.stringify(responses)};
const updates = ${JSON.stringify(updates)};
const storage = ${JSON.stringify(storage)};
globalThis.fetch = async (input, options) => {
  const url = new URL(input);
  if (url.origin === 'https://storage.example') {
    if (options.headers?.Authorization) throw new Error('GitHub token leaked to storage');
    return { ok: true, body: [Buffer.from(storage.body)] };
  }
  if (url.origin !== 'https://api.github.com') throw new Error('Unexpected API host');
  if (/\\/artifacts\\/\\d+\\/zip$/.test(url.pathname)) {
    return { status: storage.status, headers: new Headers({ location: storage.location }) };
  }
  const key = url.pathname + (url.searchParams.has('page') ? '?page=' + url.searchParams.get('page') : '');
  if (!(key in responses)) return { ok: false, status: 404 };
  const value = responses[key];
  if (key in updates) responses[key] = updates[key];
  return { ok: true, json: async () => value };
};\n`);
      return spawnSync(process.execPath, ['--import', preload, 'scripts/verify-release-ci.mjs'], {
        cwd: root, encoding: 'utf8',
        env: { ...environment, GITHUB_REPOSITORY: repository, GITHUB_SHA: commit, GH_TOKEN: 'fixture-token',
          RELEASE_PLATFORM: 'linux', GITHUB_OUTPUT: '', CI_CANDIDATE_ARCHIVE: '', ...overrides },
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
  f.responses[`${key}?page=1`] = { total_count: 7, jobs: jobs.slice(0, 3) };
  f.responses[`${key}?page=2`] = { total_count: 7, jobs: jobs.slice(3) };
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
  f.responses[key] = { ...page, total_count: 8 };
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

test('Apple Silicon release accepts its successful checks when unrelated Intel CI fails', t => {
  const f = fixture(t);
  f.run.conclusion = 'failure';
  f.responses[`${prefix}/runs/123/attempts/1/jobs?page=1`].jobs
    .find(job => job.name === 'check (macos-15-intel)').conclusion = 'failure';
  const result = f.verify({ RELEASE_PLATFORM: 'macos-aarch64' });
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

for (const [name, changes] of [
  ['a different commit', { head_sha: 'b'.repeat(40) }],
  ['a PR run rather than main integration CI', { event: 'pull_request' }],
  ['a different branch', { head_branch: 'topic' }],
  ['a different workflow', { workflow_id: 999 }],
  ['a different workflow path', { path: '.github/workflows/other.yml' }],
  ['a fork repository', { repository: { full_name: 'contributor/Mist' } }],
  ['a fork source', { head_repository: { full_name: 'contributor/Mist' } }],
  ['missing repository identity', { repository: { full_name: repository } }],
]) {
  test(`release rejects ${name} even when other CI evidence looks successful`, t => {
    const f = fixture(t);
    Object.assign(f.run, changes);
    assert.notEqual(f.verify().status, 0);
  });
}

for (const state of ['in_progress', 'failure', 'cancelled']) {
  test(`release does not depend on overall CI being green when unrelated work is ${state}`, t => {
    const f = fixture(t);
    Object.assign(f.run, state === 'in_progress'
      ? { status: 'in_progress', conclusion: null } : { conclusion: state });
    const result = f.verify();
    assert.equal(result.status, 0, result.stdout + result.stderr);
  });
}

test('release refuses to promote a build when its exact-commit CI artifact is missing', t => {
  const f = fixture(t);
  delete f.responses[`${prefix}/runs/123/artifacts?page=1`];
  const result = f.verify();
  assert.notEqual(result.status, 0, result.stdout + result.stderr);
});

test('release downloads the verified archive and emits its immutable artifact identity', t => {
  const f = fixture(t);
  const archive = path.join(f.directory, 'candidate.zip');
  const output = path.join(f.directory, 'outputs');
  const result = f.verify({ CI_CANDIDATE_ARCHIVE: archive, GITHUB_OUTPUT: output });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.equal(readFileSync(archive, 'utf8'), 'ci-candidate-fixture');
  assert.match(readFileSync(output, 'utf8'), /ci-run-id=123\nartifact-id=789\nartifact-digest=sha256:03bbb087/);
});

test('release rejects corrupted archive bytes before writing a promotable candidate', t => {
  const f = fixture(t);
  f.storage.body = 'xi-candidate-fixture';
  const archive = path.join(f.directory, 'candidate.zip');
  const result = f.verify({ CI_CANDIDATE_ARCHIVE: archive });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /checksum or size mismatch/);
  assert.equal(existsSync(archive), false);
});

test('release rejects a newer main CI run appearing during artifact verification', t => {
  const f = fixture(t);
  f.updates[`${prefix}/workflows/456/runs?page=1`] = {
    total_count: 2, workflow_runs: [f.run, { ...f.run, id: 124 }],
  };
  assert.notEqual(f.verify().status, 0);
});

for (const platform of ['', 'all', 'constructor', 'unknown']) {
  test(`release refuses invalid or missing platform ${JSON.stringify(platform)}`, t => {
    assert.notEqual(fixture(t).verify({ RELEASE_PLATFORM: platform }).status, 0);
  });
}

for (const [platform, own] of [
  ['linux', ['check (ubuntu-latest)', 'Linux package validation']],
  ['macos-aarch64', ['check (macos-latest)']],
  ['macos-x86_64', ['check (macos-15-intel)']],
  ['windows-x86_64', ['check (windows-latest)']],
]) {
  test(`${platform} release requires its own native checks but does not wait for other platforms`, t => {
    const f = fixture(t);
    const page = f.responses[`${prefix}/runs/123/attempts/1/jobs?page=1`];
    for (const job of page.jobs) {
      if (!own.includes(job.name) && job.name.startsWith('check (')) {
        job.status = 'in_progress'; job.conclusion = null;
      }
    }
    f.run.status = 'in_progress'; f.run.conclusion = null;
    let result = f.verify({ RELEASE_PLATFORM: platform });
    assert.equal(result.status, 0, result.stdout + result.stderr);
    for (const context of own) {
      const job = page.jobs.find(job => job.name === context);
      job.conclusion = 'failure';
      result = f.verify({ RELEASE_PLATFORM: platform });
      assert.notEqual(result.status, 0, context);
      job.conclusion = 'success';
    }
  });
}

for (const [name, change] of [
  ['expired flag', artifact => { artifact.expired = true; }],
  ['past expiry time', artifact => { artifact.expires_at = '2000-01-01T00:00:00Z'; }],
  ['missing digest', artifact => { delete artifact.digest; }],
  ['empty archive', artifact => { artifact.size_in_bytes = 0; }],
  ['old attempt', artifact => { artifact.name = `mist-ci-linux-${commit}-attempt-0`; }],
  ['wrong commit', artifact => { artifact.workflow_run.head_sha = 'b'.repeat(40); }],
  ['wrong run', artifact => { artifact.workflow_run.id = 999; }],
  ['PR branch', artifact => { artifact.workflow_run.head_branch = 'topic'; }],
  ['fork source', artifact => { artifact.workflow_run.head_repository_id = 999; }],
]) {
  test(`release rejects CI artifact with ${name}`, t => {
    const f = fixture(t);
    change(f.responses[`${prefix}/runs/123/artifacts?page=1`].artifacts[0]);
    assert.notEqual(f.verify().status, 0);
  });
}

test('release rejects duplicate candidate artifacts rather than picking one', t => {
  const f = fixture(t);
  const page = f.responses[`${prefix}/runs/123/artifacts?page=1`];
  page.artifacts.push({ ...page.artifacts[0], id: 999 });
  page.total_count = page.artifacts.length;
  assert.notEqual(f.verify().status, 0);
});

test('release fails closed when both run and artifact omit repository IDs', t => {
  const f = fixture(t);
  delete f.run.repository.id;
  delete f.run.head_repository.id;
  for (const artifact of f.responses[`${prefix}/runs/123/artifacts?page=1`].artifacts) {
    delete artifact.workflow_run.repository_id;
    delete artifact.workflow_run.head_repository_id;
  }
  assert.notEqual(f.verify().status, 0);
});

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
