import { env as environment } from 'node:process';
import { appendFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

const { GITHUB_REPOSITORY: repository, GITHUB_SHA: commit, GH_TOKEN: token, RELEASE_PLATFORM: platform } = environment;
const platformChecks = {
  linux: ['check (ubuntu-latest)', 'Linux package validation'],
  'macos-aarch64': ['check (macos-latest)'],
  'macos-x86_64': ['check (macos-15-intel)'],
  'windows-x86_64': ['check (windows-latest)'],
};
const sharedChecks = ['Repository metadata, workflows, and website', 'Quality and security gates'];

async function downloadCandidate(artifact) {
  if (!environment.CI_CANDIDATE_ARCHIVE) return;
  const redirect = await fetch(`https://api.github.com/repos/${repository}/actions/artifacts/${artifact.id}/zip`, {
    headers: { Authorization: `Bearer ${token}`, Accept: 'application/vnd.github+json' },
    redirect: 'manual', signal: AbortSignal.timeout(30_000),
  });
  if (redirect.status !== 302) throw new Error(`CI artifact download failed (HTTP ${redirect.status}).`);
  const location = new URL(redirect.headers.get('location'));
  if (location.protocol !== 'https:' || location.username || location.password) {
    throw new Error('Invalid CI artifact download redirect.');
  }
  // The signed storage URL does not receive the GitHub credential.
  const response = await fetch(location, { redirect: 'error', signal: AbortSignal.timeout(120_000) });
  if (!response.ok) throw new Error(`CI artifact storage failed (HTTP ${response.status}).`);
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > artifact.size_in_bytes) throw new Error('CI artifact exceeds its declared size.');
    chunks.push(chunk);
  }
  const archive = Buffer.concat(chunks);
  if (size !== artifact.size_in_bytes ||
      `sha256:${createHash('sha256').update(archive).digest('hex')}` !== artifact.digest) {
    throw new Error('CI artifact checksum or size mismatch.');
  }
  writeFileSync(environment.CI_CANDIDATE_ARCHIVE, archive, { flag: 'wx', mode: 0o600 });
}

async function api(endpoint) {
  const response = await fetch(`https://api.github.com/repos/${repository}/actions/${endpoint}`, {
    headers: { Authorization: `Bearer ${token}`, Accept: 'application/vnd.github+json' },
    signal: AbortSignal.timeout(30_000),
  });
  if (!response.ok) throw new Error(`GitHub CI lookup failed (HTTP ${response.status}).`);
  return response.json();
}

async function pages(endpoint, collection) {
  const entries = [];
  for (let page = 1; ; page += 1) {
    const result = await api(`${endpoint}${endpoint.includes('?') ? '&' : '?'}per_page=100&page=${page}`);
    if (!Number.isSafeInteger(result.total_count) || result.total_count < 0 ||
        !Array.isArray(result[collection])) throw new Error('Incomplete GitHub CI response.');
    entries.push(...result[collection]);
    if (entries.length === result.total_count) return entries;
    if (result[collection].length === 0 || entries.length > result.total_count || page >= 10) {
      throw new Error('Incomplete GitHub CI pagination.');
    }
  }
}

async function latestRun(workflow) {
  const runs = await pages(`workflows/${workflow.id}/runs?branch=main&event=push&head_sha=${commit}`, 'workflow_runs');
  if (runs.some(run => !Number.isSafeInteger(run.id) || run.id <= 0)) {
    throw new Error('Invalid GitHub CI run identity.');
  }
  const candidate = runs.sort((left, right) => right.id - left.id)[0];
  if (!candidate) throw new Error('No main CI run exists for the release commit.');
  return candidate;
}

async function verify() {
  if (!Object.hasOwn(platformChecks, platform ?? '')) throw new Error('A supported RELEASE_PLATFORM is required.');
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository ?? '') ||
      !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(commit ?? '') || !token) {
    throw new Error('Release CI verification requires a repository, exact commit SHA, and GitHub token.');
  }
  const workflow = await api('workflows/ci.yml');
  if (!Number.isSafeInteger(workflow.id) || workflow.id <= 0 ||
      workflow.state !== 'active' || workflow.path !== '.github/workflows/ci.yml') {
    throw new Error('The required CI workflow is not active.');
  }
  const candidate = await latestRun(workflow);
  const run = await api(`runs/${candidate.id}`);
  if (run.id !== candidate.id || !Number.isSafeInteger(run.run_attempt) || run.run_attempt <= 0 ||
      run.workflow_id !== workflow.id || run.path !== workflow.path ||
      run.event !== 'push' || run.head_branch !== 'main' || run.head_sha !== commit ||
      run.repository?.full_name !== repository || run.head_repository?.full_name !== repository ||
      !Number.isSafeInteger(run.repository.id) || run.repository.id <= 0 ||
      run.head_repository.id !== run.repository.id) {
    throw new Error('CI evidence does not identify the exact main commit and workflow.');
  }
  const required = [...sharedChecks, ...platformChecks[platform]];
  const jobs = await pages(`runs/${run.id}/attempts/${run.run_attempt}/jobs`, 'jobs');
  for (const check of required) {
    const matching = jobs.filter(job => job.name === check);
    if (matching.length !== 1 || matching[0].status !== 'completed' || matching[0].conclusion !== 'success' ||
        matching[0].head_sha !== commit || matching[0].run_id !== run.id || matching[0].run_attempt !== run.run_attempt) {
      throw new Error(`CI has not passed required check: ${check}.`);
    }
  }
  const artifacts = await pages(`runs/${run.id}/artifacts`, 'artifacts');
  const name = `mist-ci-${platform}-${commit}-attempt-${run.run_attempt}`;
  const matching = artifacts.filter(artifact => artifact.name === name);
  const artifact = matching[0];
  if (matching.length !== 1 || !Number.isSafeInteger(artifact.id) || artifact.id <= 0 ||
      artifact.expired !== false || !Number.isSafeInteger(artifact.size_in_bytes) ||
      artifact.size_in_bytes <= 0 || artifact.size_in_bytes > 512 * 1024 * 1024 ||
      !/^sha256:[a-f0-9]{64}$/.test(artifact.digest ?? '') ||
      !Number.isFinite(Date.parse(artifact.expires_at)) || Date.parse(artifact.expires_at) <= Date.now() ||
      artifact.workflow_run?.id !== run.id || artifact.workflow_run?.head_sha !== commit ||
      artifact.workflow_run?.head_branch !== 'main' ||
      artifact.workflow_run?.repository_id !== run.repository.id ||
      artifact.workflow_run?.head_repository_id !== run.head_repository.id) {
    throw new Error(`Missing or invalid CI candidate artifact for ${platform} in the latest attempt.`);
  }
  await downloadCandidate(artifact);
  const current = await api(`runs/${run.id}`);
  if (current.run_attempt !== run.run_attempt || current.head_sha !== commit ||
      (await latestRun(workflow)).id !== run.id) {
    throw new Error('CI changed while release evidence was being verified; retry after it completes.');
  }
  if (environment.GITHUB_OUTPUT) {
    appendFileSync(environment.GITHUB_OUTPUT,
      `ci-run-id=${run.id}\nartifact-id=${artifact.id}\nartifact-digest=${artifact.digest}\n`);
  }
  console.log(`Verified CI for ${platform}: https://github.com/${repository}/actions/runs/${run.id}`);
}

verify().catch(error => {
  console.error(error.message);
  process.exitCode = 1;
});
