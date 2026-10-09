# Mist

[![Website: Download Mist](https://img.shields.io/badge/Website-Download_Mist-2563eb)](https://mindful-time.github.io/Mist/)
[![Releases: Early access](https://img.shields.io/badge/Releases-Early_access-f59e0b)](https://github.com/mindful-time/Mist/releases)
[![GitHub release downloads](https://img.shields.io/github/downloads/mindful-time/Mist/total?label=Release%20downloads)](https://github.com/mindful-time/Mist/releases)

**Select text. Press Ctrl+Space. Listen to it.**

Mist reads selected text aloud on your computer. Use it to listen to articles,
documents, and notes from other applications.

Speech stays on your device. After the first model download, Mist can generate
speech without an internet connection.

A small animated orb stays on your desktop. It moves with the sound and becomes
smaller when speech stops.

[Download Mist](https://mindful-time.github.io/Mist/) ·
[Start using Mist](#start-using-mist) ·
[Report a problem](https://github.com/mindful-time/Mist/issues)

## What you can do

- Read selected text in supported applications.
- Choose from 54 voices.
- Set the speech speed from 0.5× to 3×.
- Pause or resume speech from the queue.
- Delete the active queue item to stop its speech.
- Move the orb and the queue to another position on your desktop.

## Download and install

Mist is an early-access release candidate. It is not a stable release.

Use the [download page](https://mindful-time.github.io/Mist/) to choose an
available installer for your computer.

The website shows available downloads for your operating system. On a Mac,
choose the processor type before you download.

| Computer | System requirements | Public download |
| --- | --- | --- |
| Apple Silicon Mac | macOS 13.0 or newer | [RC.2 DMG](https://github.com/mindful-time/Mist/releases/download/v0.1.0-rc.2-macos-aarch64/Mist-macos-aarch64.dmg) |
| Linux x86_64 | Ubuntu 24.04-compatible desktop and an AVX2-capable CPU | [RC.2 AppImage](https://github.com/mindful-time/Mist/releases/download/v0.1.0-rc.2-linux/Mist-linux-x86_64.AppImage) or [RC.2 DEB](https://github.com/mindful-time/Mist/releases/download/v0.1.0-rc.2-linux/Mist-linux-x86_64.deb) |
| Intel Mac | See the [source build requirements](docs/DISTRIBUTION.md#intel-macos-runtime) | No public installer |
| Windows | See [Build from source](#build-from-source) | No public installer |

The Apple Silicon installer is signed and notarized. Linux requires glibc 2.39+
and GLIBCXX_3.4.32. AppImage does not remove these requirements.

Read the [release notes](https://github.com/mindful-time/Mist/releases) before
installation. The published RC.2 downloads do not include later changes on
`main`.

These previews still need full installation and speech tests on clean computers.
See the [installation and checksum instructions](docs/DISTRIBUTION.md#user-installation).

## Start using Mist

1. Install the package for your computer.
2. Open Mist.
3. On macOS, enable Mist in **System Settings → Privacy & Security → Accessibility**.
4. Select **Download voices** in Mist.
5. Wait for the download to finish.
6. Open a language group on the **Voices** page.
7. Select a voice.

The first download is approximately 338 MiB. Mist stores the model and voice
files on your computer. Mist verifies their SHA-256 checksums.

Mist plays a sample when you select a voice. Choose a voice that matches the
language of your text.

## Read selected text

1. Select text in a supported application.
2. Press **Ctrl+Space**.

Mist adds the text to the speech queue. It starts speech automatically unless
you disable automatic playback in **Settings → Playback**.

Use the pause control to pause speech. Use the resume control to continue
speech. Use the delete control to remove an item. If you delete the active item,
Mist stops its speech.

Right-click the orb to open its menu. Select **Settings** to change the voice,
speed, or playback mode.

### If Mist cannot read the selection

1. Copy the text in the application.
2. Right-click the Mist orb.
3. Select **Speak copied text**.

On macOS, you can also use **Services → Speak Selection with Mist** in
applications that support Services.

On Wayland, your desktop must approve the shortcut. Use the shortcut that your
desktop grants. Some desktops cannot provide selected text to Mist.

On Windows and Linux, **Speak typed text** also accepts text that you enter.

## Voices and playback

Mist uses the [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) speech model.
Its 54 voices have nine language groups:

- English (US)
- English (UK)
- Spanish
- French
- Hindi
- Italian
- Japanese
- Portuguese (BR)
- Mandarin

Select another voice to replace the current voice sample. The new voice applies
to subsequent speech.

Open **Settings → Playback** to choose when speech starts:

- **Real-time** starts speech after the first sentence is ready. This is the
  default mode.
- **Complete audio** prepares all selected text before speech starts.

Turn off **Automatically play new queue items** to start each item yourself.

## Privacy and permissions

Mist generates and plays speech on your computer. It does not send selected text
to a speech service. It uses the internet to download model and voice files.

Mist first asks the operating system for selected text. If direct access fails,
Mist can use the Copy shortcut. This option is on by default.

Automatic Copy can replace text that you previously copied. Open **Settings →
Privacy**. Turn off **Use Copy when selection access fails** to disable this
option.

After speech, Mist clears its temporary clipboard text only if the clipboard
still matches. Clipboard cleanup cannot fully protect text copied at the same
time.

Mist does not request permission to record the screen.

## Current limits

- Some applications restrict access to selected text.
- Wayland selection and shortcuts depend on desktop support.
- Mist does not start automatically when you log in.
- GPU acceleration on Windows and Linux still needs tests on supported physical
  hardware.

See the [platform and runtime details](docs/INFERENCE_BACKEND_RESEARCH.md) for
tested capabilities and remaining limits.

## Build from source

Use Rust 1.95 or newer. The first build and model download require internet
access.

- On macOS, install the Xcode Command Line Tools. Intel Mac also needs the
  [pinned runtime preparation](docs/DISTRIBUTION.md#intel-macos-runtime).
- On Windows, use Rust's `stable-msvc` toolchain.
- On Linux, install the [desktop build libraries](.github/workflows/ci.yml).
  Install DejaVu Sans, Noto Core, and Noto CJK fonts. Install `pw-play`, `paplay`,
  `aplay`, or `ffplay` for audio output.

Run the tests:

```sh
make test
```

Start Mist on macOS or Linux:

```sh
make run
```

Build and start Mist in Windows PowerShell:

```powershell
cargo build --release --locked
.\target\release\mist.exe
```

For a macOS development install, use a stable Apple certificate. Repeated ad-hoc
builds can require new Accessibility permission. Avoid simultaneous development
and release installations.

See the [distribution guide](docs/DISTRIBUTION.md) for source installation,
runtime requirements, and command-line options.

## Contribute

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you change the code. Install the
repository hooks:

```sh
make hooks
```

All changes require a pull request with the required CI checks. Only
`mindful-time` can merge changes to `main`.

For technical details, read the [architecture guide](docs/ARCHITECTURE.md) and
[product specification](docs/SPEC.md).

Mist uses the [MIT license](LICENSE). The speech engine, models, and bundled
fonts have separate license terms. See the [license details](docs/LICENSING_RESEARCH.md).
