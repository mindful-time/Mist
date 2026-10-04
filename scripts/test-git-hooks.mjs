import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const source = path.resolve(import.meta.dirname, '..');

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'mist-hook-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
  git('init', '--quiet');
  git('config', 'user.email', 'fixture@example.invalid');
  git('config', 'user.name', 'Hook fixture');
  git('config', 'core.hooksPath', '.githooks');
  mkdirSync(path.join(root, 'scripts'));
  for (const name of ['pre-commit.sh', 'pre-push.sh', 'check-git-snapshot.sh']) {
    try { copyFileSync(path.join(source, 'scripts', name), path.join(root, 'scripts', name)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  // The snapshot runner launches the repository's quality process. This fixture
  // substitutes that process, not Git or its index/commit snapshot semantics.
  writeFileSync(path.join(root, 'scripts/check-quality.sh'), '#!/bin/sh\nset -eu\ntest "$(cat marker)" = good\n');
  writeFileSync(path.join(root, 'marker'), 'good\n');
  git('add', '.');
  git('-c', 'core.hooksPath=/dev/null', 'commit', '--quiet', '-m', 'fixture');
  return { root, git, run: (name, input = '') => spawnSync('sh', [path.join(root, 'scripts', name)], {
    cwd: root, encoding: 'utf8', input,
  }) };
}

test('pre-commit checks the staged snapshot without changing local edits', t => {
  const f = fixture(t);
  writeFileSync(path.join(f.root, 'marker'), 'bad\n');
  const before = f.git('status', '--porcelain');
  const result = f.run('pre-commit.sh');
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.equal(f.git('status', '--porcelain'), before);
  assert.equal(readFileSync(path.join(f.root, 'marker'), 'utf8'), 'bad\n');
});

test('an unstaged repair cannot hide bad staged code', t => {
  const f = fixture(t);
  writeFileSync(path.join(f.root, 'marker'), 'bad\n');
  f.git('add', 'marker');
  writeFileSync(path.join(f.root, 'marker'), 'good\n');
  const before = f.git('status', '--porcelain');
  assert.notEqual(f.run('pre-commit.sh').status, 0);
  assert.equal(f.git('status', '--porcelain'), before);
});

test('pre-push checks the pushed commit, not the index or working tree', t => {
  const f = fixture(t);
  const good = f.git('rev-parse', 'HEAD');
  writeFileSync(path.join(f.root, 'marker'), 'bad\n');
  f.git('add', 'marker');
  f.git('-c', 'core.hooksPath=/dev/null', 'commit', '--quiet', '-m', 'bad commit');
  const bad = f.git('rev-parse', 'HEAD');
  writeFileSync(path.join(f.root, 'marker'), 'good\n');
  f.git('add', 'marker');
  assert.notEqual(f.run('pre-push.sh', `refs/heads/topic ${bad} refs/heads/topic ${good}\n`).status, 0);
  assert.equal(f.run('pre-push.sh', `refs/heads/topic ${good} refs/heads/topic ${'0'.repeat(40)}\n`).status, 0);
});

test('quality runs every requested gate and fails closed on incomplete scans', t => {
  const f = fixture(t);
  copyFileSync(path.join(source, 'scripts/check-quality.sh'), path.join(f.root, 'scripts/check-quality.sh'));
  for (const name of ['test-distribution.sh', 'release-check.sh', 'build-site.sh', 'crap-gate.sh']) {
    writeFileSync(path.join(f.root, 'scripts', name), `#!/bin/sh\nprintf '%s\\n' '${name}' >> "$MIST_GATE_TRACE"\n`);
  }
  const bin = path.join(f.root, 'bin');
  mkdirSync(bin);
  for (const name of ['cargo', 'uv', 'uvx', 'osv-scanner', 'gitleaks', 'jq', 'node']) {
    writeFileSync(path.join(bin, name), `#!/bin/sh\nprintf '%s\\n' '${name}'" $*" >> "$MIST_GATE_TRACE"\nif [ "\${MIST_FAIL_TOOL:-}" = '${name}' ]; then exit 2; fi\n`, { mode: 0o755 });
  }
  const trace = path.join(f.root, 'trace');
  // Deliberately do not inherit credentials or the surrounding shell environment.
  const run = fail => spawnSync('sh', [path.join(f.root, 'scripts/check-quality.sh')], {
    cwd: f.root, encoding: 'utf8', env: { PATH: `${bin}:/usr/bin:/bin`, MIST_GATE_TRACE: trace, MIST_FAIL_TOOL: fail },
  });
  const result = run('');
  assert.equal(result.status, 0, result.stderr);
  const commands = readFileSync(trace, 'utf8');
  for (const required of ['cargo fmt --all -- --check', 'cargo check --locked --all-targets',
    'cargo clippy --locked --all-targets -- -D warnings', 'cargo build --locked --all-targets',
    'cargo test --locked --all-targets', 'test-distribution.sh', 'crap-gate.sh',
    'uvx --from smells==0.5.0 smells check --path .', 'osv-scanner scan source --lockfile Cargo.lock',
    'gitleaks git --redact .']) assert.ok(commands.includes(required), required);
  for (const fail of ['cargo', 'uvx', 'osv-scanner', 'gitleaks']) assert.equal(run(fail).status, 2, fail);
});
