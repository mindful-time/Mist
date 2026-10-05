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

## Intel macOS runtime follow-up

Approved on 2026-10-04 after the Intel candidate failed at `ort-sys`. Review
these changes from merged `main`, `1c505dc67c5e2814e8514034b4374c4c8481db25`.
See [the runtime research](INTEL_MAC_SUPPORT_RESEARCH.md).

- Keep the locked `ort`/`ort-sys` 2.0.0-rc.13 APIs and existing model/voices.
  Build ONNX Runtime 1.28.0 from its exact source commit on native Intel macOS;
  begin with CPU inference and static linkage. Do not downgrade the Windows
  DirectML implementation or change Apple Silicon's Core ML runtime path.
- Share the Intel runtime build between CI and Release. Cache only an exact
  source/build-script/compiler/SDK configuration, preserve build provenance,
  and include ONNX Runtime's license and third-party notices in the Intel app.
- Make Intel native builds, application/audio tests, and real multilingual
  synthesis part of read-only CI. Validate the release-mode app's architecture,
  deployment target, and self-contained static linkage without signing secrets.
  Release consumes exact-commit CI evidence; it must not rerun application tests.
- The Intel candidate targets macOS 13.3. Align its bundle metadata with that
  target and reject unexpected runtime library dependencies or binary minimums.
  A runner on macOS 15 is not acceptance evidence for macOS 13.3.
- Use a new PR branch; leave merging and server-ruleset application to the owner.
  Do not publish a release, create a tag, change signing credentials, or enable
  the download website. The separate notarization credential error, Windows
  signing, and clean-machine acceptance remain outstanding.

## Linux-first preview and honest website availability

Requested on 2026-10-05: fix the unavailable-download UI and provide the software
that is available while the other platform release work continues. Review this
work against `f8b796ec5cd4fb6d5ea53dd68770bb792872213c`.

- Offer Linux first as an explicitly labeled early-access preview, not a
  complete stable desktop release. Mac and Windows remain coming soon.
- Use protected `v<base-version>-linux-preview.<positive number>` tags on main,
  exact-commit successful integration CI, the existing native Linux packaging
  action, attestations, and protected draft-only publication. Linux preview
  preparation must not require Apple or Windows signing configuration.
- Keep the complete desktop release, its five-platform-asset contract, both
  signing paths, main/CI rules, and protected publication unchanged. Do not
  publish an unsigned Mac/Windows installer or claim clean-machine acceptance.
- The website prefers a stable release; only if none exists may it offer a
  published Linux preview. Never expose drafts, unfinished uploads, or external
  download URLs. An unavailable platform must not have a download CTA or
  installation steps. No release means coming soon, not a download promise.
- Test the tag classifier through its CLI, workflow dependencies through the
  actual YAML, and user-facing download states through the actual site script
  and markup with browser/HTTP boundaries supplied by fixtures.
- Submit this work via a new PR and preserve owner-only merging. Do not tag the
  preview before the new workflow is merged and its main CI passes. Protected
  environment approval and draft review remain maintainer actions. Do not
  change credentials or server protection rules.

## Independent platform releases and inclusive CRAP limits

Requested on 2026-10-05. Review these changes against merged main,
`8dd7108ace1b138a4155cf980bfa32263ae53d2d`. This supersedes the Linux-only
preview discovery above, not the CI, signing, or owner-merge requirements.

- Provide separate Linux, Apple Silicon macOS, Intel macOS, and Windows
  workflows. Each platform builds and prepares its own protected draft preview
  without waiting for another platform's release build/signing job.
- Share native build/signing logic through a same-commit local reusable workflow.
  Manual candidates run from main and upload Actions artifacts only. Protected
  platform tags use the synchronized stable base version and positive preview
  numbers. Require all exact-commit main CI checks before any release build;
  audio, quality, and security tests remain in CI and hooks, not Release.
- Match tags to the selected platform, preserve Apple notarization/stapling and
  Windows publisher verification, and publish only that platform's artifacts,
  matching bootstrap, icon, and checksums. Reject missing/empty/extra/symlink
  artifacts or missing required bootstrap identities. Linux needs no signing
  credentials. Keep attestations and protected draft-only publication.
- The optional complete desktop release still builds all five platform assets
  and generates the existing Homebrew/Chocolatey recipes; it must not run for
  platform preview tags or duplicate their publication.
- Before a stable desktop release exists, the website aggregates the newest
  valid published preview per platform channel. Stable releases take precedence.
  Ignore drafts, invalid publication dates, unfinished/empty uploads, wrong-
  channel assets, and external URLs. Keep unavailable downloads and install
  instructions disabled and retain an early-access label.
- Warn at CRAP >=5, fail new/regressed scores >=10, and reject a crossing from
  below 10 even within analyzer epsilon. Preserve the existing-debt baseline;
  unchanged/improving debt remains visible. Fail closed on incomplete analyzer
  evidence. Do not regenerate the baseline or suppress Smells findings.
- Test the actual workflow graph, CLI tag/assets/CRAP boundaries, and actual
  website script/markup with only external boundaries substituted. Commit/push
  a new PR for the owner to merge. Do not merge, tag, publish, change credentials
  or protection rules, or claim clean-machine/native release acceptance.

## Shared release-candidate version

Clarified on 2026-10-05: all native installers, Homebrew, and Chocolatey use
`0.1.0-rc.1`, not `0.0.1` or a stable release. Review from
`f7c041cd3a691a57c3760db9293c0229f9214a98`.

- Synchronize VERSION, Cargo.toml/Cargo.lock, and the full Mac release identity.
  Keep Apple's numeric CFBundleShortVersionString at the base version and the
  build number equal to the RC number; do not insert SemVer prerelease text into
  those keys. Normalize DEB candidates to `x.y.z~rc.N` before checksums, preserving
  their payload and source artifact, so they upgrade to stable in Debian order.
- Preserve independent platform pipelines, existing signing/CI/environment
  protections, and draft-only creation. Use `v<RC-version>-<platform>` tags;
  the common `v<RC-version>` tag remains the optional complete bundle. Never
  run both publication paths for one platform tag or overwrite published assets.
- Discover published common and platform RCs on the website, show their real
  version, preserve stable precedence and channel/URL/upload checks, and keep
  unavailable platforms disabled. Legacy platform previews remain discoverable.
- Generate complete-bundle Homebrew and Chocolatey recipes for the same RC
  version with version-pinned URLs/checksums/publisher. Recipe generation is not
  tap/feed publication. Missing signing credentials and native acceptance remain
  blockers; no unsigned Mac/Windows release or false package-manager commands.
- Submit version/pipeline/site changes through a PR. Leave its merge to the
  owner and do not create candidate tags against the old main workflow.
