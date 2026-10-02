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
