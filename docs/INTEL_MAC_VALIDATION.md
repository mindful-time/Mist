# Intel macOS native CI validation

Verified on 2026-10-04 for [PR #3](https://github.com/mindful-time/Mist/pull/3).
This records an unsigned CI candidate, not a signed or published release.

## Tested source and CI

- PR head: `5893c88cc64a37a976d89e241a01a2ed9d6214c1`.
- Tested PR merge ref: `66ab9cff66d1c76d5eb259eb482d195ca328c769`.
- [CI run 37205358084](https://github.com/mindful-time/Mist/actions/runs/37205358084)
  completed successfully, with all seven jobs passing.
- [Native Intel job](https://github.com/mindful-time/Mist/actions/runs/37205358084/job/111445306912)
  passed Cargo check, build, Clippy, and 112 ordinary application/audio tests.
  Its separately executed production-model integration test also passed.
- The production-model test asserted the CPU backend and generated streamed
  English, Spanish, Japanese, and Mandarin speech with the existing voices.
  Each voice had nonempty chunks, a 24,000 Hz sample rate, finite samples, and
  at least one sample with amplitude greater than 0.0001. Console output reports
  test success, not per-language measurements or subjective listening quality.
- The pinned CPU runtime builder completed its cold build in approximately
  29 minutes 56 seconds. The exact runtime cache was saved before app checks.

This is PR CI evidence. Release must still verify successful push-to-`main` CI
for the exact merged release commit; this run cannot authorize a release.

## Inspected artifact

Downloaded [artifact 11305976079](https://github.com/mindful-time/Mist/actions/runs/37205358084/artifacts/11305976079),
`mist-intel-ci-66ab9cff66d1c76d5eb259eb482d195ca328c769`, and matched the
downloaded outer ZIP's SHA-256 to GitHub's artifact digest:

```text
f4b40d679f9618a2e2183b71f654df9b16fc72360e1634dbef96ccf5b3f598c6
```

Both ZIP layers passed integrity checks; their paths were inspected before
extraction. The inner `Mist-intel-ci.zip` contains `Mist.app`. Without executing
the Intel app on the Apple Silicon inspection host, verified:

- Mach-O architecture: `x86_64` only.
- Mach-O minimum OS and bundle `LSMinimumSystemVersion`: macOS 13.3.
- Dynamic dependencies: only `/System/Library/` and `/usr/lib/`; no external
  ONNX Runtime library or build-machine dependency.
- Runtime license, third-party notices, and provenance included in the app.
- Provenance: ONNX Runtime 1.28.0, source
  `da9b5e364c465de65c49d91e696cd6485270757f`, CPU backend, Xcode 16.4,
  Apple Clang 17.0.0, SDK 15.5, and source/configuration/archive hashes.
- `codesign` reports that the bundle is not signed, as required for this
  read-only CI validation path without signing credentials.

## Remaining release gates

The owner must add `check (macos-15-intel)` to the existing live main-quality
ruleset and merge the PR. Editing the versioned template does not apply it to
GitHub. Main integration CI must then succeed on the exact release commit.

Before public downloads: verify a signed, notarized and stapled macOS candidate;
configure and validate Windows signing; and complete clean-machine acceptance
on every supported platform, including Intel macOS 13.3. Native CI on macOS 15
does not establish execution on 13.3, audible playback, subjective audio quality,
or acceptable latency and memory usage. Public release, package-registry
publication, and website deployment remain separate owner-authorized steps.
