use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use std::io::ErrorKind;

use anyhow::{Context, Result, bail};

use crate::{
    domain::{Audio, AudioFeatures},
    ports::AudioPlayer,
};

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
    fn start_player(&self) -> Result<Child> {
        Command::new("/usr/bin/afplay")
            .arg(&self.output_path)
            .spawn()
            .context("could not start macOS audio playback")
    }

    #[cfg(target_os = "windows")]
    fn start_player(&self) -> Result<Child> {
        Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$player = New-Object System.Media.SoundPlayer $env:MIST_WAV; $player.PlaySync()",
            ])
            .env("MIST_WAV", &self.output_path)
            .spawn()
            .context("could not start Windows audio playback")
    }

    #[cfg(target_os = "linux")]
    fn play_file(
        &self,
        audio: &Audio,
        on_sample: &mut dyn FnMut(AudioFeatures),
    ) -> Result<ExitStatus> {
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
                .spawn()
            {
                Ok(mut child) => {
                    let status = monitor_playback(&mut child, audio, on_sample)
                        .with_context(|| format!("could not monitor {program} playback"))?;
                    if status.success() {
                        return Ok(status);
                    }
                    playback_failures.push(format!("{program} exited with {status}"));
                }
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
    fn play(&mut self, audio: &Audio, on_sample: &mut dyn FnMut(AudioFeatures)) -> Result<()> {
        self.write_wav(audio)?;
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let status = {
            let mut child = self.start_player()?;
            monitor_playback(&mut child, audio, on_sample)
                .context("could not monitor system audio playback")?
        };
        #[cfg(target_os = "linux")]
        let status = self.play_file(audio, on_sample)?;
        if !status.success() {
            bail!("system audio playback exited with {status}");
        }
        Ok(())
    }
}

fn monitor_playback(
    child: &mut Child,
    audio: &Audio,
    on_sample: &mut dyn FnMut(AudioFeatures),
) -> Result<ExitStatus> {
    const SAMPLE_INTERVAL: Duration = Duration::from_millis(40);
    const FEATURE_WINDOW: Duration = Duration::from_millis(80);

    let started = Instant::now();
    loop {
        on_sample(audio.features_at(started.elapsed(), FEATURE_WINDOW));
        if let Some(status) = child
            .try_wait()
            .context("could not query system audio playback")?
        {
            return Ok(status);
        }
        thread::sleep(SAMPLE_INTERVAL);
    }
}
