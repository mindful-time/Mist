# Distribution and contributor-gate requirements

Implementation scope agreed on 2026-10-03, reviewed from commit
`2382714ffb81b0a339263abdff82679356c4efc4`.

- Keep `main` PR-only for everyone. Only `mindful-time` may merge; required
  checks cannot be bypassed by that account. Contributors use forks and PRs.
- Pre-commit must check the complete staged snapshot, preserving unstaged edits.
  Pre-push must check the actual revisions supplied by Git, not local HEAD/index.
- Both hooks must run formatting, compile checks, Clippy, actual build, tests,
  coverage-backed CRAP, pinned Smells 0.5.0, OSV, and redacted Gitleaks. Preserve
  the existing warning/block thresholds and documented audit exceptions; do not
  suppress findings, regenerate baseline debt, or weaken policy to pass.
- Enforce the full quality/security suite in required CI, with pinned tools,
  full history, and retained coverage/CRAP/Smells evidence. Native build/tests
  continue on macOS, Windows, and Linux. Do not confuse coverage on one platform
  with clean-machine or native GPU acceptance on all platforms.
- Generate a dual-architecture Homebrew cask and Chocolatey package around the
  existing signed DMGs and NSIS EXE. Pin release version, hashes, and Windows
  publisher. Refuse missing/corrupt/ambiguous inputs and unconfigured signer.
- Test package generation and installer trust decisions at their CLI boundary,
  including corrupted, unsigned, and unexpected-publisher rejection. Preserve
  curl bootstraps, Linux packages, signing/notarization, protected environments,
  and draft-only publication. MSI is not required for this iteration.
- Document setup and distinguish prepared recipes from published packages.
  Signing credentials and clean-machine acceptance remain release prerequisites.
- Commit and push a feature branch, open a PR, and leave merging to the owner.
  Do not create release tags, public releases, a tap repository, Chocolatey
  submissions, credential changes, or website deployment during this task.

The linked Smells chat proposes a future staged debt-envelope comparison.
Published Smells 0.5.0 already reads complete staged blobs but does not implement
that comparison. This work must not claim to have changed the separate scanner.

## PR/release separation follow-up

Approved on 2026-10-03, reviewed from PR #2's existing commit
`b4560172a8afc68ab6d0e89caa8a5887a4ebd80e`.

- CI runs on PRs targeting `main` and pushes to `main`. Do not start a duplicate
  CI run for a PR branch push or run Release on a PR.
- Move Linux production-package validation into required read-only CI, retaining
  ABI, dependency, license, and archive checks. Reuse the same build/validation
  logic in the release path.
- Keep Release for version-tag pushes and manual candidates from `main`.
  Validate the release source/version, then rerun the full fail-closed quality
  suite on that exact commit before any platform builds/signing. Preserve
  protected signing/publishing environments, package formats, and draft-only
  publication; do not change signing credentials or create a release.
- Replace the existing required `Linux release packages` context with
  `Linux package validation` one-for-one in the template and live ruleset after
  observing the new CI check. Preserve all other main protections.
- Add regression tests for workflow triggers, the required read-only package
  check, the release quality dependency, and pinned release checkouts. Keep
  Smells 0.5.0 and the existing
  policies/CRAP baseline; do not invent the proposed staged debt comparison.
- Update PR #2, leave it unmerged, and leave publication/website deployment alone.

## CI-owned testing and release consumption follow-up

Approved on 2026-10-03 after the user clarified that audio tests belong in CI
and Release owns building/distribution. Review these additions from
`ec80e552e8cd08a271a503fb93097169fac4ba31` in the existing PR #2.
This supersedes the earlier requirement to rerun quality inside Release.

- Keep audio/application tests, CRAP, Smells, OSV, Gitleaks, and native validation
  in required CI and the existing local hooks. Do not weaken thresholds or
  regenerate the CRAP baseline to repair the timing-sensitive audio coverage.
- Release validates its source/version, then requires successful canonical
  push-to-`main` CI for the exact release SHA. Verify the latest run/attempt and
  every required job from the versioned main-quality ruleset. Reject missing,
  failed, skipped, pending, PR/fork, wrong-commit, stale, or incomplete evidence;
  API/network failures also block. Do not rerun application tests in Release.
- Test the verifier through its CLI with GitHub responses substituted only at
  the external HTTP boundary; test the actual workflow dependency graph and
  repair audio process tests using real child-process state.
- Preserve pinned release checkouts, source/version checks, package formats,
  native signing/notarization, protected environments, and draft-only publication.
  The website links published release assets and stays opt-in after acceptance.
- Commit and push to PR #2. Leave merging, tags, public publication, signing
  credential setup, package-registry submissions, and website deployment pending
  their existing prerequisites; no direct-main push or protection bypass.
