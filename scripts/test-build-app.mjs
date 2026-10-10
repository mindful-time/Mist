import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const source = path.resolve(import.meta.dirname, '..');
const notices = [
  ['LICENSE', 'Mist-LICENSE'],
  ['third_party/kokoro-micro/LICENSE', 'kokoro-micro-LICENSE'],
  ['third_party/kokoro-micro/NOTICE', 'kokoro-micro-NOTICE'],
  ['third_party/kokoro-micro/THIRD_PARTY_LICENSES', 'kokoro-micro-THIRD_PARTY_LICENSES'],
  ['assets/fonts/OFL-NotoSansDevanagariUI.txt', 'NotoSansDevanagariUI-OFL'],
];

function fixture(t) {
  const temporary = mkdtempSync(path.join(tmpdir(), 'mist-preview-notices-'));
  t.after(() => rmSync(temporary, { recursive: true, force: true }));
  const root = path.join(temporary, 'preview project');
  const files = ['scripts/build-app.sh', 'scripts/build-macos-icon.sh', 'scripts/prepare-intel-app.sh',
    'macos/Info.plist', 'assets/mist-orb-v1.png', ...notices.map(([file]) => file)];
  for (const file of files) {
    const destination = path.join(root, file);
    mkdirSync(path.dirname(destination), { recursive: true });
    copyFileSync(path.join(source, file), destination);
  }
  const binary = path.join(root, 'target/release/mist');
  mkdirSync(path.dirname(binary), { recursive: true });
  const architecture = spawnSync('uname', ['-m'], { encoding: 'utf8' });
  assert.equal(architecture.status, 0, architecture.stderr);
  const compiled = spawnSync('xcrun', ['clang', '-arch', architecture.stdout.trim(),
    '-mmacosx-version-min=13.3', '-x', 'c', '-', '-o', binary], {
    input: 'int main(void) { return 0; }\n', encoding: 'utf8',
  });
  assert.equal(compiled.status, 0, compiled.stderr);
  const bin = path.join(root, 'bin');
  mkdirSync(bin);
  // Substitute only Cargo's compile boundary. Bundle assembly, icon generation,
  // Intel validation and ad-hoc signing all run their actual implementations.
  writeFileSync(path.join(bin, 'cargo'), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  const runtime = path.join(root, 'runtime');
  mkdirSync(runtime);
  for (const file of ['LICENSE', 'ThirdPartyNotices.txt', 'provenance.txt']) {
    writeFileSync(path.join(runtime, file), `Intel runtime fixture: ${file}\n`);
  }
  const app = path.join(root, 'dist/Mist.app');
  return { root, app, run: () => spawnSync('zsh', [path.join(root, 'scripts/build-app.sh')], {
    encoding: 'utf8', env: {
      PATH: `${bin}:/usr/bin:/bin:/usr/sbin:/sbin`, MIST_SIGNING_IDENTITY: '-',
      MIST_INTEL_ORT_SOURCE: runtime, ORT_LIB_PATH: runtime,
    },
  }) };
}

test('preview bundles retain exact notices inside their native signature', { skip: process.platform !== 'darwin' }, t => {
  const f = fixture(t);
  const result = f.run();
  assert.equal(result.status, 0, result.stdout + result.stderr);
  const licenses = path.join(f.app, 'Contents/Resources/licenses');
  for (const [file, name] of notices) {
    assert.deepEqual(readFileSync(path.join(licenses, name)), readFileSync(path.join(source, file)), name);
  }
  const verify = () => spawnSync('/usr/bin/codesign', ['--verify', '--deep', '--strict', f.app], { encoding: 'utf8' });
  assert.equal(verify().status, 0);
  writeFileSync(path.join(licenses, 'kokoro-micro-NOTICE'), 'changed after signing\n');
  assert.notEqual(verify().status, 0, 'the signature must seal the bundled notices');
});
