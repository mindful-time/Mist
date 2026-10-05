import { createHash } from 'node:crypto';
import { lstatSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const artifacts = {
  linux: ['Mist-linux-x86_64.deb', 'Mist-linux-x86_64.AppImage'],
  'macos-aarch64': ['Mist-macos-aarch64.dmg'],
  'macos-x86_64': ['Mist-macos-x86_64.dmg'],
  'windows-x86_64': ['Mist-windows-x86_64-setup.exe'],
};

try {
  const [platform, directory, ...extra] = process.argv.slice(2);
  if (!Object.hasOwn(artifacts, platform) || !directory || extra.length) {
    throw new Error('Usage: prepare-preview-release.mjs PLATFORM ASSETS_DIRECTORY');
  }
  const expected = artifacts[platform];
  const names = readdirSync(directory).sort();
  if (JSON.stringify(names) !== JSON.stringify([...expected].sort())) {
    throw new Error('Preview must contain exactly the selected platform artifacts.');
  }
  for (const name of names) {
    const info = lstatSync(join(directory, name));
    if (!info.isFile() || info.size === 0) throw new Error(`Missing, empty, or non-regular artifact: ${name}`);
  }

  const windows = platform === 'windows-x86_64';
  const installer = windows ? 'mist-installer.ps1' : 'mist-installer.sh';
  let bootstrap = readFileSync(new URL(windows ? './install-release.ps1' : './install-release.sh', import.meta.url), 'utf8');
  if (windows) {
    const fingerprint = process.env.WINDOWS_SIGNER_SHA256;
    if (!/^[a-fA-F0-9]{64}$/.test(fingerprint ?? '')) throw new Error('WINDOWS_SIGNER_SHA256 must be a certificate SHA-256 fingerprint.');
    bootstrap = bootstrap.replaceAll('__MIST_WINDOWS_SIGNER_SHA256__', fingerprint.toUpperCase());
  } else if (platform.startsWith('macos-')) {
    const team = process.env.APPLE_TEAM_ID;
    if (!/^[A-Z0-9]{10}$/.test(team ?? '')) throw new Error('APPLE_TEAM_ID must be a 10-character Team ID.');
    bootstrap = bootstrap.replaceAll('__MIST_APPLE_TEAM_ID__', team);
  }
  const icon = readFileSync(new URL('../assets/mist-orb-512-v1.png', import.meta.url));
  // Validate all inputs before preparing a publishable bundle. Signature and
  // native package checks are performed by the successful selected build job.
  writeFileSync(join(directory, installer), bootstrap.replaceAll('\r\n', '\n'), { flag: 'wx' });
  writeFileSync(join(directory, 'Mist.png'), icon, { flag: 'wx' });
  const checksums = [...names, installer, 'Mist.png'].sort().map(name => {
    const hash = createHash('sha256').update(readFileSync(join(directory, name))).digest('hex');
    return `${hash}  ${name}\n`;
  }).join('');
  writeFileSync(join(directory, 'SHA256SUMS'), checksums, { flag: 'wx' });
  console.log(`Prepared ${platform} preview with verified checksums.`);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
