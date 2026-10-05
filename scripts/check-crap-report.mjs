import { readFileSync } from 'node:fs';

function readEntries(file) {
  const report = JSON.parse(readFileSync(file, 'utf8'));
  if (!Array.isArray(report.entries) || report.entries.length === 0) throw new Error('Missing CRAP entries');
  for (const entry of report.entries) {
    if (typeof entry.file !== 'string' || typeof entry.function !== 'string' ||
        !Number.isSafeInteger(entry.line) || entry.line < 1 ||
        !Number.isFinite(entry.crap) || entry.crap < 0) throw new Error('Invalid CRAP entry');
  }
  return report.entries;
}

function escapeAnnotation(value, property = false) {
  const escaped = String(value).replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
  return property ? escaped.replaceAll(':', '%3A').replaceAll(',', '%2C') : escaped;
}

try {
  const [currentPath, deltaPath, ...extra] = process.argv.slice(2);
  if (!currentPath || !deltaPath || extra.length) throw new Error('Usage: check-crap-report.mjs current.json delta.json');
  const current = readEntries(currentPath);
  const delta = readEntries(deltaPath);
  const identity = entry => JSON.stringify([entry.file, entry.function, entry.line]);
  const scores = new Map(current.map(entry => [identity(entry), entry.crap]));
  const seen = new Set();
  for (const entry of delta) {
    if (!['new', 'regressed', 'improved', 'unchanged'].includes(entry.status) ||
        scores.get(identity(entry)) !== entry.crap || seen.has(identity(entry)) ||
        (entry.status !== 'new' && (!Number.isFinite(entry.baseline_crap) || entry.baseline_crap < 0))) {
      throw new Error('Incomplete or inconsistent CRAP delta');
    }
    seen.add(identity(entry));
  }
  if (scores.size !== current.length || seen.size !== scores.size) throw new Error('Incomplete CRAP delta');

  for (const entry of current.filter(entry => entry.crap >= 5)) {
    console.error(`::warning file=${escapeAnnotation(entry.file, true)},line=${entry.line},title=CRAP (>=5)::${escapeAnnotation(entry.function)} has CRAP score ${entry.crap}`);
  }
  // A baseline crossing is an error even if cargo-crap's epsilon labels the
  // tiny increase "unchanged". Existing debt still ratchets rather than resets.
  const blockers = delta.filter(entry => entry.crap >= 10 &&
    (entry.status === 'new' || entry.status === 'regressed' || entry.baseline_crap < 10));
  if (blockers.length) {
    console.error(JSON.stringify({ gate: 'crap', result: 'blocked', warning_threshold: 5, blocking_threshold: 10,
      policy: 'New, regressed, or newly boundary-crossing CRAP scores of 10 or higher block; existing debt is ratcheted.',
      evidence_report: deltaPath }));
    for (const entry of blockers) {
      console.error(`::error file=${escapeAnnotation(entry.file, true)},line=${entry.line},title=CRAP (>=10)::${escapeAnnotation(entry.function)} has blocking CRAP score ${entry.crap}`);
      console.error(`BLOCK ${entry.file}:${entry.line} ${entry.function} CRAP=${entry.crap} status=${entry.status}`);
    }
    process.exitCode = 1;
  } else {
    console.log(`CRAP gate passed: warn at 5 or higher, block new/regressed at 10 or higher, report=${currentPath}`);
  }
} catch (error) {
  console.error(`CRAP gate incomplete: ${error.message}`);
  process.exitCode = 2;
}
