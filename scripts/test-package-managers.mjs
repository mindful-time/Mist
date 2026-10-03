import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const generator = path.join(import.meta.dirname, 'build-package-managers.mjs');
const digest = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'; // SHA-256 known-answer: abc
const emptyDigest = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';
const assets = ['Mist-macos-aarch64.dmg', 'Mist-macos-x86_64.dmg', 'Mist-windows-x86_64-setup.exe'];
const signer = 'A1'.repeat(32);

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'mist-packages-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const name of assets) writeFileSync(path.join(root, name), 'abc');
  writeFileSync(path.join(root, assets[1]), '');
  writeFileSync(path.join(root, 'SHA256SUMS'), assets.map((name, i) => `${i === 1 ? emptyDigest : digest}  ${name}\n`).join(''));
  const output = path.join(root, 'packages');
  return { root, output, run: (version = '0.1.0', fingerprint = signer) => spawnSync(process.execPath,
    [generator, version, root, output, fingerprint], { encoding: 'utf8' }) };
}

test('release recipes pin version, architecture checksums, and Windows publisher', t => {
  const f = fixture(t);
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  const cask = readFileSync(path.join(f.output, 'Casks/mist.rb'), 'utf8');
  assert.match(cask, /version "0\.1\.0"/);
  assert.match(cask, /arch arm: "aarch64", intel: "x86_64"/);
  assert.ok(cask.includes(`sha256 arm: "${digest}", intel: "${emptyDigest}"`));
  assert.ok(cask.includes('releases/download/v#{version}/Mist-macos-#{arch}.dmg'));
  assert.match(cask, /depends_on macos: :ventura/);
  const script = readFileSync(path.join(f.output, 'chocolatey/tools/chocolateyinstall.ps1'), 'utf8');
  assert.ok(script.includes(`$expectedChecksum = '${digest}'`));
  assert.ok(script.includes(`$expectedSigner = '${signer}'`));
  assert.ok(script.includes('releases/download/v0.1.0/Mist-windows-x86_64-setup.exe'));
  assert.ok(!cask.includes('__MIST_') && !script.includes('__MIST_'));
  if (process.platform === 'darwin') {
    const syntax = spawnSync('ruby', ['-c', path.join(f.output, 'Casks/mist.rb')], { encoding: 'utf8' });
    assert.equal(syntax.status, 0, syntax.stderr);
  }
});

test('corrupted or missing release artifacts cannot produce usable recipes', t => {
  const f = fixture(t);
  writeFileSync(path.join(f.root, assets[0]), 'corrupted');
  assert.notEqual(f.run().status, 0);
  assert.equal(existsSync(f.output), false);
  rmSync(path.join(f.root, assets[0]));
  assert.notEqual(f.run().status, 0);
  assert.equal(existsSync(f.output), false);
});

test('invalid version or unpinned publisher is rejected before output', t => {
  const f = fixture(t);
  for (const version of ['v0.1.0', '0.1.0;exit', '0.1.0-beta', '00.1.0']) {
    assert.notEqual(f.run(version).status, 0);
  }
  assert.notEqual(f.run('0.1.0', '__MIST_WINDOWS_SIGNER_SHA256__').status, 0);
  assert.equal(existsSync(f.output), false);
});

test('ambiguous checksum entries are rejected', t => {
  const f = fixture(t);
  const manifest = path.join(f.root, 'SHA256SUMS');
  writeFileSync(manifest, readFileSync(manifest, 'utf8') + `${digest}  ${assets[0]}\n`);
  assert.notEqual(f.run().status, 0);
  assert.equal(existsSync(f.output), false);
});

test('conflicting binary and text checksum records are also ambiguous', t => {
  const f = fixture(t);
  const manifest = path.join(f.root, 'SHA256SUMS');
  writeFileSync(manifest, `${'0'.repeat(64)} *${assets[0]}\n` + readFileSync(manifest, 'utf8'));
  assert.notEqual(f.run().status, 0);
  assert.equal(existsSync(f.output), false);
});
