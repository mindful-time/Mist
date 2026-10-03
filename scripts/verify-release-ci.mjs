import { env as environment } from 'node:process';
import { readFileSync } from 'node:fs';

const { GITHUB_REPOSITORY: repository, GITHUB_SHA: commit, GH_TOKEN: token } = environment;

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

async function verify() {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository ?? '') ||
      !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(commit ?? '') || !token) {
    throw new Error('Release CI verification requires a repository, exact commit SHA, and GitHub token.');
  }
  const workflow = await api('workflows/ci.yml');
  if (!Number.isSafeInteger(workflow.id) || workflow.id <= 0 ||
      workflow.state !== 'active' || workflow.path !== '.github/workflows/ci.yml') {
    throw new Error('The required CI workflow is not active.');
  }
  const runs = await pages(`workflows/${workflow.id}/runs?branch=main&event=push&head_sha=${commit}`, 'workflow_runs');
  if (runs.some(run => !Number.isSafeInteger(run.id) || run.id <= 0)) {
    throw new Error('Invalid GitHub CI run identity.');
  }
  const candidate = runs.sort((left, right) => right.id - left.id)[0];
  if (!candidate) throw new Error('No main CI run exists for the release commit.');
  const run = await api(`runs/${candidate.id}`);
  if (run.id !== candidate.id || !Number.isSafeInteger(run.run_attempt) || run.run_attempt <= 0 ||
      run.workflow_id !== workflow.id || run.path !== workflow.path ||
      run.event !== 'push' || run.head_branch !== 'main' || run.head_sha !== commit ||
      run.repository?.full_name !== repository || run.head_repository?.full_name !== repository) {
    throw new Error('CI evidence does not identify the exact main commit and workflow.');
  }
  if (run?.status !== 'completed' || run.conclusion !== 'success') {
    throw new Error('Release requires successful CI for the exact main commit.');
  }
  const ruleset = JSON.parse(readFileSync(new URL('../.github/rulesets/main-quality.json', import.meta.url), 'utf8'));
  const required = ruleset.rules.find(rule => rule.type === 'required_status_checks').parameters.required_status_checks;
  if (!Array.isArray(required) || required.length === 0) throw new Error('No required CI checks are configured.');
  const jobs = await pages(`runs/${run.id}/attempts/${run.run_attempt}/jobs`, 'jobs');
  for (const check of required) {
    const matching = jobs.filter(job => job.name === check.context);
    if (matching.length !== 1 || matching[0].status !== 'completed' || matching[0].conclusion !== 'success' ||
        matching[0].head_sha !== commit || matching[0].run_id !== run.id || matching[0].run_attempt !== run.run_attempt) {
      throw new Error(`CI has not passed required check: ${check.context}.`);
    }
  }
  const current = await api(`runs/${run.id}`);
  if (current.run_attempt !== run.run_attempt || current.status !== 'completed' || current.conclusion !== 'success') {
    throw new Error('CI changed while release evidence was being verified; retry after it completes.');
  }
  console.log(`Verified CI: https://github.com/${repository}/actions/runs/${run.id}`);
}

verify().catch(error => {
  console.error(error.message);
  process.exitCode = 1;
});
