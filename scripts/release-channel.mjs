const [version, tag, ...extra] = process.argv.slice(2);

if (extra.length || !/^\d+\.\d+\.\d+(?:[+-][0-9A-Za-z.-]+)?$/.test(version ?? '')) {
  console.error('A synchronized release version and tag are required.');
  process.exitCode = 1;
} else if (tag === `v${version}`) {
  console.log('desktop');
} else if (/^\d+\.\d+\.\d+$/.test(version) && tag?.startsWith(`v${version}-linux-preview.`) &&
           /^[1-9]\d*$/.test(tag.slice(`v${version}-linux-preview.`.length))) {
  console.log('linux-preview');
} else {
  console.error(`Tag must match v${version} or v${version}-linux-preview.<positive number>.`);
  process.exitCode = 1;
}
