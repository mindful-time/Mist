import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const checker = readFileSync(new URL('./release-check.sh', import.meta.url), 'utf8');

function fixture(t, version = '0.1.0-rc.1') {
  const root = mkdtempSync(path.join(tmpdir(), 'mist-version-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const directory of ['scripts', 'src', 'macos']) mkdirSync(path.join(root, directory));
  writeFileSync(path.join(root, 'scripts/release-check.sh'), checker);
  writeFileSync(path.join(root, 'VERSION'), `${version}\n`);
  writeFileSync(path.join(root, 'Cargo.toml'), `[package]\nname = "mist"\nversion = "${version}"\nedition = "2024"\n`);
  writeFileSync(path.join(root, 'Cargo.lock'), `version = 4\n\n[[package]]\nname = "mist"\nversion = "${version}"\n`);
  writeFileSync(path.join(root, 'src/lib.rs'), '');
  const plist = `<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0"><dict>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>MistReleaseVersion</key>
    <string>${version}</string>
  </dict></plist>\n`;
  const file = path.join(root, 'macos/Info.plist');
  writeFileSync(file, plist);
  return { root, file, plist, run: () => spawnSync('sh', ['scripts/release-check.sh'], { cwd: root, encoding: 'utf8' }) };
}

test('release metadata accepts the shared RC version with an Apple-compatible numeric bundle version', t => {
  const f = fixture(t);
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /0\.1\.0-rc\.1 is synchronized/);
});

test('a final stable release keeps the same numeric bundle-version contract', t => {
  const f = fixture(t, '0.1.0');
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
});

test('mismatched Cargo, bundle, or RC identity blocks the release', t => {
  const f = fixture(t);
  for (const content of [
    f.plist.replace('<string>0.1.0</string>', '<string>0.1.0-rc.1</string>'),
    f.plist.replace('<string>0.1.0-rc.1</string>', '<string>0.1.0-rc.2</string>'),
    f.plist.replace('<key>MistReleaseVersion</key>', '<key>UnrelatedKey</key>'),
    f.plist.replace('<string>1</string>', '<string>2</string>'),
  ]) {
    writeFileSync(f.file, content);
    assert.notEqual(f.run().status, 0);
  }
  writeFileSync(f.file, f.plist);
  writeFileSync(path.join(f.root, 'VERSION'), '0.1.0-rc.2\n');
  assert.notEqual(f.run().status, 0);
});
