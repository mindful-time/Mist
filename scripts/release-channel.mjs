const [version, tag, ...extra] = process.argv.slice(2);
const preview = tag?.match(/^v(\d+\.\d+\.\d+)-(linux|macos-aarch64|macos-x86_64|windows-x86_64)-preview\.([1-9]\d*)$/);

if (extra.length || !/^\d+\.\d+\.\d+(?:[+-][0-9A-Za-z.-]+)?$/.test(version ?? '')) {
  console.error('A synchronized release version and tag are required.');
  process.exitCode = 1;
} else if (tag === `v${version}`) {
  console.log('desktop');
} else if (/^\d+\.\d+\.\d+$/.test(version) && preview?.[1] === version) {
  console.log(`${preview[2]}-preview`);
} else {
  console.error(`Tag must match v${version} or v${version}-<supported platform>-preview.<positive number>.`);
  process.exitCode = 1;
}
