import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const command = fileURLToPath(new URL('./prepare-preview-release.mjs', import.meta.url));
const team = 'N229MA78TX';
const signer = 'ab'.repeat(32);
const platforms = {
  linux: ['Mist-linux-x86_64.deb', 'Mist-linux-x86_64.AppImage'],
  'macos-aarch64': ['Mist-macos-aarch64.dmg'],
  'macos-x86_64': ['Mist-macos-x86_64.dmg'],
  'windows-x86_64': ['Mist-windows-x86_64-setup.exe'],
};

function fixture(t, platform) {
  const directory = mkdtempSync(join(tmpdir(), 'mist-preview-test-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  for (const name of platforms[platform] ?? []) writeFileSync(join(directory, name), `fixture ${name}`);
  return {
    directory,
    run: (overrides = {}) => spawnSync(process.execPath, [command, platform, directory], {
      encoding: 'utf8', env: { APPLE_TEAM_ID: '', WINDOWS_SIGNER_SHA256: '', ...overrides },
    }),
  };
}

for (const platform of Object.keys(platforms)) {
  test(`${platform} preview contains only its own installers and verified checksums`, t => {
    const { directory, run } = fixture(t, platform);
    const result = run({ APPLE_TEAM_ID: team, WINDOWS_SIGNER_SHA256: signer });
    assert.equal(result.status, 0, result.stderr);
    const installer = platform === 'windows-x86_64' ? 'mist-installer.ps1' : 'mist-installer.sh';
    const names = [...platforms[platform], 'Mist.png', installer].sort();
    assert.deepEqual(readdirSync(directory).sort(), [...names, 'SHA256SUMS'].sort());
    const expected = names.map(name => `${createHash('sha256').update(readFileSync(join(directory, name))).digest('hex')}  ${name}\n`).join('');
    assert.equal(readFileSync(join(directory, 'SHA256SUMS'), 'utf8'), expected);
    const bootstrap = readFileSync(join(directory, installer), 'utf8');
    if (platform.startsWith('macos-')) {
      assert.ok(bootstrap.includes(team));
      assert.ok(!bootstrap.includes('__MIST_APPLE_TEAM_ID__'));
    } else if (platform === 'windows-x86_64') {
      assert.ok(bootstrap.includes(signer.toUpperCase()));
      assert.ok(!bootstrap.includes('__MIST_WINDOWS_SIGNER_SHA256__'));
    } else {
      assert.ok(bootstrap.includes('__MIST_APPLE_TEAM_ID__'), 'unconfigured Mac trust remains closed');
    }
  });
}

test('Linux preview needs no Apple or Windows credentials', t => {
  assert.equal(fixture(t, 'linux').run().status, 0);
});

for (const platform of ['macos-aarch64', 'macos-x86_64', 'windows-x86_64']) {
  test(`${platform} refuses unconfigured bootstrap identity without preparing assets`, t => {
    const { directory, run } = fixture(t, platform);
    assert.notEqual(run().status, 0);
    assert.deepEqual(readdirSync(directory).sort(), platforms[platform]);
    assert.notEqual(run({ APPLE_TEAM_ID: 'wrong', WINDOWS_SIGNER_SHA256: 'wrong' }).status, 0);
  });
}

for (const invalid of ['missing', 'empty', 'extra', 'symlink']) {
  test(`preview refuses ${invalid} artifacts before writing bootstrap or checksums`, t => {
    const { directory, run } = fixture(t, 'linux');
    const path = join(directory, platforms.linux[0]);
    if (invalid === 'missing' || invalid === 'symlink') rmSync(path);
    if (invalid === 'empty') writeFileSync(path, '');
    if (invalid === 'extra') writeFileSync(join(directory, 'Mist-macos-aarch64.dmg'), 'wrong platform');
    if (invalid === 'symlink') symlinkSync(platforms.linux[1], path);
    assert.notEqual(run().status, 0);
    assert.ok(!readdirSync(directory).includes('SHA256SUMS'));
    assert.ok(!readdirSync(directory).includes('mist-installer.sh'));
  });
}

test('unknown platforms fail closed', t => {
  assert.notEqual(fixture(t, 'android').run().status, 0);
});
