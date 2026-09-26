use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

#[cfg(target_os = "linux")]
use std::io::ErrorKind;

use anyhow::{Context, Result, bail};

use crate::{domain::Audio, ports::AudioPlayer};

/// Cross-platform system audio adapter. Keeping playback behind this port avoids
/// pulling device APIs into the application core.
pub struct SystemAudioPlayer {
    output_path: PathBuf,
}

impl SystemAudioPlayer {
    pub fn new(cache_directory: &Path) -> Result<Self> {
        fs::create_dir_all(cache_directory).with_context(|| {
            format!(
                "could not create audio cache at {}",
                cache_directory.display()
            )
        })?;
        Ok(Self {
            output_path: cache_directory.join("selection.wav"),
        })
    }

    fn write_wav(&self, audio: &Audio) -> Result<()> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: audio.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&self.output_path, spec)
            .context("could not create the temporary speech file")?;
        for sample in &audio.samples {
            let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            writer
                .write_sample(pcm)
                .context("could not write speech audio")?;
        }
        writer.finalize().context("could not finish speech audio")
    }

    #[cfg(target_os = "macos")]
    fn play_file(&self) -> Result<ExitStatus> {
        Command::new("/usr/bin/afplay")
            .arg(&self.output_path)
            .status()
            .context("could not start macOS audio playback")
    }

    #[cfg(target_os = "windows")]
    fn play_file(&self) -> Result<ExitStatus> {
        Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$player = New-Object System.Media.SoundPlayer $env:SELECT_TO_SPEAK_WAV; $player.PlaySync()",
            ])
            .env("SELECT_TO_SPEAK_WAV", &self.output_path)
            .status()
            .context("could not start Windows audio playback")
    }

    #[cfg(target_os = "linux")]
    fn play_file(&self) -> Result<ExitStatus> {
        let candidates: &[(&str, &[&str])] = &[
            ("pw-play", &[]),
            ("paplay", &[]),
            ("aplay", &[]),
            ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
        ];

        let mut playback_failures = Vec::new();
        for (program, arguments) in candidates {
            match Command::new(program)
                .args(*arguments)
                .arg(&self.output_path)
                .status()
            {
                Ok(status) if status.success() => return Ok(status),
                Ok(status) => playback_failures.push(format!("{program} exited with {status}")),
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(error) => return Err(error).context("could not start Linux audio playback"),
            }
        }

        if !playback_failures.is_empty() {
            bail!(
                "Linux audio playback failed: {}",
                playback_failures.join("; ")
            );
        }
        bail!("no audio player found; install PipeWire (pw-play), PulseAudio, ALSA, or ffplay")
    }
}

impl AudioPlayer for SystemAudioPlayer {
    fn play(&mut self, audio: &Audio) -> Result<()> {
        self.write_wav(audio)?;
        let status = self.play_file()?;
        if !status.success() {
            bail!("system audio playback exited with {status}");
        }
        Ok(())
    }
}
