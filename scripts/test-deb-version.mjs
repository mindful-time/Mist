import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const script = path.join(import.meta.dirname, 'prepare-deb.sh');
const hasDebianTools = spawnSync('dpkg-deb', ['--version']).status === 0;

function fixture(t, version) {
  const root = mkdtempSync(path.join(tmpdir(), 'mist-deb-version-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const packageRoot = path.join(root, 'package');
  mkdirSync(path.join(packageRoot, 'DEBIAN'), { recursive: true });
  mkdirSync(path.join(packageRoot, 'usr/share/mist'), { recursive: true });
  writeFileSync(path.join(packageRoot, 'DEBIAN/control'),
    `Package: mist\nVersion: ${version}\nArchitecture: amd64\nMaintainer: Mist <maintainer@example.invalid>\nDescription: Known test package\n`);
  writeFileSync(path.join(packageRoot, 'usr/share/mist/payload'), 'known payload\n');
  const input = path.join(root, 'input.deb');
  const output = path.join(root, 'output.deb');
  const built = spawnSync('dpkg-deb', ['--build', '--root-owner-group', packageRoot, input], { encoding: 'utf8' });
  assert.equal(built.status, 0, built.stderr);
  return { root, input, output, run: (target = version) =>
    spawnSync('sh', [script, input, output, target], { encoding: 'utf8' }) };
}

test('the actual DEB RC sorts before stable and retains its original payload', { skip: !hasDebianTools }, t => {
  const f = fixture(t, '0.1.0-rc.1');
  const before = readFileSync(f.input);
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(readFileSync(f.input), before);
  const version = spawnSync('dpkg-deb', ['--field', f.output, 'Version'], { encoding: 'utf8' });
  assert.equal(version.stdout.trim(), '0.1.0~rc.1');
  assert.equal(spawnSync('dpkg', ['--compare-versions', version.stdout.trim(), 'lt', '0.1.0']).status, 0);
  const extracted = path.join(f.root, 'extracted');
  assert.equal(spawnSync('dpkg-deb', ['--extract', f.output, extracted]).status, 0);
  assert.equal(readFileSync(path.join(extracted, 'usr/share/mist/payload'), 'utf8'), 'known payload\n');
});

test('stable DEB versions are not changed', { skip: !hasDebianTools }, t => {
  const f = fixture(t, '0.1.0');
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(spawnSync('dpkg-deb', ['--field', f.output, 'Version'], { encoding: 'utf8' }).stdout.trim(), '0.1.0');
});

test('wrong source versions and malformed RC versions cannot prepare a package', { skip: !hasDebianTools }, t => {
  const f = fixture(t, '0.1.0-rc.1');
  for (const version of ['0.1.0-rc.2', '0.1.0', '0.1.0-rc.01', '0.1.0-rc.0', '0.1.0-rc.1;echo']) {
    assert.notEqual(f.run(version).status, 0);
    assert.equal(existsSync(f.output), false);
  }
});

test('existing artifacts and symlink inputs are never replaced or followed', { skip: !hasDebianTools }, t => {
  const f = fixture(t, '0.1.0-rc.1');
  writeFileSync(f.output, 'existing artifact');
  assert.notEqual(f.run().status, 0);
  assert.equal(readFileSync(f.output, 'utf8'), 'existing artifact');
  const link = path.join(f.root, 'linked.deb');
  symlinkSync(f.input, link);
  const result = spawnSync('sh', [script, link, path.join(f.root, 'new.deb'), '0.1.0-rc.1']);
  assert.notEqual(result.status, 0);
});
