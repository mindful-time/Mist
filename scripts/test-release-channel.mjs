import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const command = fileURLToPath(new URL('./release-channel.mjs', import.meta.url));

for (const platform of ['linux', 'macos-aarch64', 'macos-x86_64', 'windows-x86_64']) {
  test(`${platform} accepts RC.2 only when its source version also identifies RC.2`, () => {
    const tag = `v0.1.0-rc.2-${platform}`;
    const result = spawnSync(process.execPath, [command, '0.1.0-rc.2', tag], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim(), `${platform}-preview`);
    assert.notEqual(spawnSync(process.execPath, [command, '0.1.0-rc.1', tag]).status, 0);
  });

  test(`${platform} uses the shared release-candidate version`, () => {
    const result = spawnSync(process.execPath,
      [command, '0.1.0-rc.1', `v0.1.0-rc.1-${platform}`], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim(), `${platform}-preview`);
    for (const tag of [`v0.1.0-rc.2-${platform}`, `v0.1.0-${platform}`, `v0.1.0-rc.1-${platform}-extra`]) {
      assert.notEqual(spawnSync(process.execPath, [command, '0.1.0-rc.1', tag]).status, 0);
    }
  });
}

test('a common RC tag cannot select an all-platform release', () => {
  const result = spawnSync(process.execPath, [command, '0.1.0-rc.1', 'v0.1.0-rc.1'], { encoding: 'utf8' });
  assert.notEqual(result.status, 0);
});

test('Linux preview tags are distinct from the complete desktop release', () => {
  const result = spawnSync(process.execPath, [command, '0.1.0', 'v0.1.0-linux-preview.1'], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), 'linux-preview');
});

for (const platform of ['macos-aarch64', 'macos-x86_64', 'windows-x86_64']) {
  test(`${platform} has its own synchronized preview tag`, () => {
    const result = spawnSync(process.execPath, [command, '0.1.0', `v0.1.0-${platform}-preview.1`], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim(), `${platform}-preview`);
  });
}

test('a common stable tag cannot select an all-platform release', () => {
  const result = spawnSync(process.execPath, [command, '0.1.0', 'v0.1.0'], { encoding: 'utf8' });
  assert.notEqual(result.status, 0);
});

for (const tag of ['v0.2.0', 'v0.2.0-linux-preview.1', 'v0.1.0-linux-preview.0',
  'v0.1.0-linux-preview.01', 'v0.1.0-linux-preview.-1', 'v0.1.0-linux-preview.1-extra',
  'v0.1.0-windows-preview.1', 'v0.1.0-linux-preview.', 'main']) {
  test(`release refuses unsupported or mismatched tag ${tag}`, () => {
    assert.notEqual(spawnSync(process.execPath, [command, '0.1.0', tag]).status, 0);
  });
}

test('Linux previews use a stable base version, matching website discovery', () => {
  assert.notEqual(spawnSync(process.execPath, [command, '0.1.0-beta.1', 'v0.1.0-beta.1-linux-preview.1']).status, 0);
});
