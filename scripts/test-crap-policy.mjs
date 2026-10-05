import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const command = path.resolve(import.meta.dirname, 'check-crap-report.mjs');

function check(t, score, status = 'new', baseline = null, throughGate = false) {
  const root = mkdtempSync(path.join(tmpdir(), 'mist-crap-policy-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const entry = { file: './src/example.rs', function: 'example', line: 12, crap: score };
  const current = path.join(root, 'current.json');
  const delta = path.join(root, 'delta.json');
  writeFileSync(current, JSON.stringify({ entries: [entry] }));
  writeFileSync(delta, JSON.stringify({ entries: [{ ...entry, status, baseline_crap: baseline }] }));
  if (throughGate) {
    for (const directory of ['scripts', 'quality', 'bin']) mkdirSync(path.join(root, directory));
    for (const name of ['crap-gate.sh', 'check-crap-report.mjs']) {
      copyFileSync(path.join(import.meta.dirname, name), path.join(root, 'scripts', name));
    }
    writeFileSync(path.join(root, 'quality/crap-baseline.json'), '{}');
    // Cargo/LLVM are the analysis boundary. The real gate and report policy run
    // unchanged, with deterministic coverage-backed analyzer output supplied.
    writeFileSync(path.join(root, 'bin/cargo'), `#!/bin/sh
set -eu
if [ "$*" = 'crap --version' ]; then echo 'cargo-crap 0.5.0'; exit 0; fi
kind=$1
shift
output=
delta=false
format=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output|--output-path) output=$2; shift ;;
    --baseline) delta=true; shift ;;
    --format) format=$2; shift ;;
  esac
  shift
done
if [ "$kind" = llvm-cov ]; then : > "$output"; exit 0; fi
if [ "$format" = github ]; then exit 0; fi
if [ "$delta" = true ]; then cp "$MIST_CRAP_DELTA" "$output"; else cp "$MIST_CRAP_CURRENT" "$output"; fi
`, { mode: 0o755 });
    writeFileSync(path.join(root, 'bin/rustc'), '#!/bin/sh\nif [ "$1" = -vV ]; then echo "host: fixture"; else echo /nonexistent; fi\n', { mode: 0o755 });
    return spawnSync('sh', [path.join(root, 'scripts/crap-gate.sh')], { encoding: 'utf8', env: {
      PATH: `${path.join(root, 'bin')}:${process.env.PATH}`, MIST_CRAP_CURRENT: current,
      MIST_CRAP_DELTA: delta, MIST_QUALITY_REPORT_DIR: path.join(root, 'reports'),
    } });
  }
  return spawnSync(process.execPath, [command, current, delta], { encoding: 'utf8' });
}

test('a new function at CRAP 10 fails rather than falling through the boundary', t => {
  const result = check(t, 10);
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /BLOCK .*CRAP=10/);
  assert.match(result.stderr, /::error file=.*title=CRAP \(>=10\)::/);
});

test('the actual coverage-backed gate rejects a new score of exactly 10', t => {
  const result = check(t, 10, 'new', null, true);
  assert.equal(result.status, 1, result.stdout + result.stderr);
  assert.match(result.stderr, /BLOCK .*CRAP=10/);
});

for (const score of [4.999, 5, 9.999]) {
  test(`CRAP ${score} stays below the error boundary with warnings starting at 5`, t => {
    const result = check(t, score);
    assert.equal(result.status, 0, result.stderr);
    if (score >= 5) assert.match(result.stderr, /::warning/);
    else assert.equal(result.stderr, '');
  });
}

test('worsening existing high-risk code blocks, but unchanged debt remains visible', t => {
  assert.equal(check(t, 20, 'regressed', 19).status, 1);
  const unchanged = check(t, 20, 'unchanged', 20);
  assert.equal(unchanged.status, 0, unchanged.stderr);
  assert.match(unchanged.stderr, /::warning/);
  assert.equal(check(t, 10, 'improved', 11).status, 0);
});

test('a tiny boundary crossing cannot escape through the analyzer epsilon', t => {
  assert.equal(check(t, 10, 'unchanged', 9.999).status, 1);
});

test('malformed analyzer evidence fails closed instead of passing the gate', t => {
  const result = check(t, null);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /incomplete/);
});
