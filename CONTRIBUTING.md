# Contributing to Mist

All changes go through a feature branch and pull request, including the
maintainer's changes. Only `mindful-time` can merge to `main`; the maintainer
has no direct-push or quality-check bypass. Contributions from other accounts
should use forks. Do not push tags or publish releases as part of a contribution.

## Install the local checks

Use Rust 1.95 with rustfmt and Clippy, Node.js 24+, uv, jq, OSV-Scanner 2.3.8,
and Gitleaks 8.30.1. Install the Rust analysis tools and hooks:

```sh
rustup component add rustfmt clippy llvm-tools-preview
cargo install cargo-llvm-cov --version 0.8.7 --locked
cargo install cargo-crap --version 0.5.0 --locked
make hooks
```

Homebrew Rust users need Homebrew LLVM instead of rustup's LLVM component.
The coverage script discovers `/opt/homebrew/opt/llvm` automatically.
Smells is run through `uvx --from smells==0.5.0`; no unpinned global scanner
is used. Linux also needs the desktop headers listed in the CI workflow.

## What commit and push check

Both hooks run formatting, compile checks, strict Clippy, an actual build,
Rust tests, distribution regression tests, release-version checks, website
build/syntax checks, coverage-backed CRAP, Smells, OSV, and redacted Gitleaks.
These are deliberately comprehensive and can take several minutes.

Pre-commit freezes the entire Git index in a disposable local clone. Unstaged
and untracked files cannot repair or mask staged code. It leaves your working
tree and index untouched. Pre-push reads Git's stdin revisions and checks each
non-deleted pushed tip, not the current working tree; it scans newly pushed
history for secrets (full history for a new ref). Fetch first if the remote
base commit is missing locally. Intermediate commits receive the history secret
scan, not separate build/coverage runs. `make quality` checks the working tree
directly; `make test` runs Rust and distribution tests.

Reports from hooks remain under `target/quality/index-<tree>` or
`target/quality/commit-<commit>`. Smells exit 1 and exit 2 both block. Read its
Issue Index and evidence before acting on findings, following `AGENTS.md`.
Review signals are not proven bugs. The published 0.5.0 scanner checks the
complete staged snapshot; it does not implement the proposed debt-envelope
comparison from the separate Smells project.

CRAP uses actual LLVM test coverage. Scores of 5 or higher warn; new or regressed
scores of 10 or higher block against `quality/crap-baseline.json`. Crossing from
below 10 to 10 or higher also blocks, even within the analyzer's epsilon. Do not update that
baseline, weaken policy, or suppress findings merely to make checks pass.
Rust CRAP/Smells do not measure JavaScript, shell, or PowerShell; those have
explicit CLI, syntax, and installer trust-decision tests instead.

Local hooks are opt-in and can be bypassed with Git options. GitHub's required
CI checks are the merge enforcement. The `Quality and security gates` job runs
the same suite with pinned scanners, full Git history, and retained evidence.
Coverage runs on Apple Silicon to match the committed CRAP baseline; native
builds and tests are separate required jobs on macOS, Linux, and Windows.
The required `Linux package validation` job builds and inspects production
DEB/AppImage packages in CI without running the Release workflow. CI runs for
PRs targeting `main` and pushes to `main`, not duplicate pushes to PR branches
or release tags.
Fork PRs do not receive signing credentials. Only the owner may apply changes
to the existing server rulesets; editing `.github/rulesets/` alone does not
change GitHub enforcement.

Workflow-boundary regression tests parse the actual YAML and branch-rules
template. They run through `uv` with locked PyYAML 6.0.3 as part of
`make test`, both hooks, and required CI. OSV checks both Cargo dependencies
and this test-only Python lockfile.

## Release work

See [distribution](docs/DISTRIBUTION.md),
[package managers](docs/PACKAGE_MANAGER_DISTRIBUTION.md), and
[Apple signing](docs/APPLE_SIGNING.md). A successful PR is not evidence of a
signed release or clean-machine acceptance. Never commit credentials or publish
an unsigned stable release. A pre-release website may show unavailable downloads
or clearly labeled platform previews, following the distribution guide.
Linux, Apple Silicon Mac, Intel Mac, and Windows have independent release
workflows using shared native build/signing logic. One release failure does not
block another platform's draft preview; all still require successful main CI.
Release runs only for version-tag pushes or manual candidate runs from `main`.
Tests and quality/security analysis belong to CI. Before packaging or signing,
Release verifies the latest push-to-`main` CI run for its exact commit and every
required check from `.github/rulesets/main-quality.json`. A PR merge-ref result,
another commit, a failed/skipped check, or unavailable evidence cannot authorize
a release. Rerun all integration-CI jobs on `main` if evidence is incomplete;
Release does not rerun the test suite or substitute another successful run.
Protected signing/publishing environments and draft-only publication remain.
The Linux packaging composite is shared between CI validation and Release.

Intel macOS additionally uses the revision-pinned source build described
in [distribution](docs/DISTRIBUTION.md#intel-macos-runtime). Set its absolute
`ORT_LIB_PATH` before running local checks; ordinary `ort-sys` downloads do not
contain an Intel Mac runtime. The `check (macos-15-intel)` job uses the same
CPU runtime preparation as Release, validates an unsigned release-mode app,
and runs the downloaded-model multilingual smoke test. It receives no signing
credentials. The versioned ruleset and Release verifier require this check;
only the owner may add its new context to the existing server ruleset.
