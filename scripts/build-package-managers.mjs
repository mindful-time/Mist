import { createHash } from 'node:crypto';
import { lstatSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const [version, assetDirectory, outputDirectory, signer] = process.argv.slice(2);
try {
  if (process.argv.length !== 6 || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
    throw new Error('Usage: node scripts/build-package-managers.mjs <stable-version> <release-assets> <output> <Windows-certificate-SHA256>');
  }
  if (!/^[a-fA-F0-9]{64}$/.test(signer)) throw new Error('A pinned Windows certificate SHA-256 is required.');
  const assets = path.resolve(assetDirectory);
  const output = path.resolve(outputDirectory);
  const root = path.resolve(import.meta.dirname, '..');
  const manifest = readFileSync(path.join(assets, 'SHA256SUMS'), 'utf8').split(/\r?\n/).filter(Boolean).map(line => {
    // Accept both GNU text (two spaces) and binary (space/star) records. Reject
    // malformed records rather than overlooking a conflicting artifact entry.
    const record = /^([a-fA-F0-9]{64}) [ *](\S+)$/.exec(line);
    if (!record) throw new Error('Malformed release checksum record.');
    return { hash: record[1].toLowerCase(), name: record[2] };
  });
  const replacements = { VERSION: version, WINDOWS_SIGNER: signer.toUpperCase() };
  for (const [key, name] of [
    ['ARM_SHA256', 'Mist-macos-aarch64.dmg'],
    ['INTEL_SHA256', 'Mist-macos-x86_64.dmg'],
    ['WINDOWS_SHA256', 'Mist-windows-x86_64-setup.exe'],
  ]) {
    const file = path.join(assets, name);
    if (!lstatSync(file).isFile()) throw new Error(`Not a regular release artifact: ${name}`);
    const hash = createHash('sha256').update(readFileSync(file)).digest('hex');
    const matches = manifest.filter(record => record.name === name);
    if (matches.length !== 1 || matches[0].hash !== hash) {
      throw new Error(`Missing, duplicate, or incorrect release checksum: ${name}`);
    }
    replacements[key] = hash;
  }
  const templates = [
    ['packaging/homebrew/mist.rb.in', 'Casks/mist.rb'],
    ['packaging/chocolatey/mist-tts.nuspec.in', 'chocolatey/mist-tts.nuspec'],
    ['packaging/chocolatey/tools/chocolateyinstall.ps1.in', 'chocolatey/tools/chocolateyinstall.ps1'],
  ];
  const files = templates.map(([input, destination]) => {
    const content = readFileSync(path.join(root, input), 'utf8').replace(/__([A-Z0-9_]+)__/g, (_, key) => {
      if (!(key in replacements)) throw new Error(`Unknown package placeholder: ${key}`);
      return replacements[key];
    });
    return [destination, content];
  });
  files.push(['chocolatey/tools/LICENSE.txt', readFileSync(path.join(root, 'LICENSE'), 'utf8')]);
  files.push(['chocolatey/tools/VERIFICATION.txt', `Source: https://github.com/mindful-time/Mist/releases/tag/v${version}\nInstaller SHA-256: ${replacements.WINDOWS_SHA256}\nPublisher certificate SHA-256: ${replacements.WINDOWS_SIGNER}\nThe install script verifies both checksum and Authenticode publisher before executing the NSIS installer with /S.\n`]);
  for (const [name, content] of files) {
    const destination = path.join(output, name);
    mkdirSync(path.dirname(destination), { recursive: true });
    writeFileSync(destination, content, { flag: 'wx' });
  }
  console.log(`Generated version-pinned package recipes in ${output}`);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
