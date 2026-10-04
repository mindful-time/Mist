//! Cross-platform system-player implementation of the core audio port.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use std::io::ErrorKind;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::{Child, Command, ExitStatus};

use anyhow::{Context, Result, bail};

use crate::{
    domain::{Audio, AudioFeatures},
    playback::{PlaybackController, PlaybackPhase, PlaybackStopped},
    ports::AudioPlayer,
};

/// Cross-platform system audio adapter. Keeping playback behind this port avoids
/// pulling device APIs into the application core.
pub struct SystemAudioPlayer {
    output_path: PathBuf,
    playback: PlaybackController,
}

impl SystemAudioPlayer {
    pub fn new(cache_directory: &Path, playback: PlaybackController) -> Result<Self> {
        fs::create_dir_all(cache_directory).with_context(|| {
            format!(
                "could not create audio cache at {}",
                cache_directory.display()
            )
        })?;
        Ok(Self {
            output_path: cache_directory.join("selection.wav"),
            playback,
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
                    let status = monitor_playback(&mut child, audio, on_sample, &self.playback)
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

    #[cfg(target_os = "windows")]
    fn play_windows(&self, audio: &Audio, on_sample: &mut dyn FnMut(AudioFeatures)) -> Result<()> {
        const ALIAS: &str = "mist_speech";
        const SAMPLE_INTERVAL: Duration = Duration::from_millis(40);
        const FEATURE_WINDOW: Duration = Duration::from_millis(80);

        let _ = mci_command(&format!("close {ALIAS}"));
        mci_command(&format!(
            "open \"{}\" type waveaudio alias {ALIAS}",
            self.output_path.display()
        ))?;
        let _session = MciSession(ALIAS);
        self.playback.begin_playback()?;
        mci_command(&format!("play {ALIAS}"))?;

        let mut elapsed = Duration::ZERO;
        let mut last_tick = Instant::now();
        let mut player_paused = false;
        loop {
            let now = Instant::now();
            let tick = now.saturating_duration_since(last_tick);
            last_tick = now;
            let phase = self.playback.phase();
            match phase {
                PlaybackPhase::Cancelled => {
                    mci_command(&format!("stop {ALIAS}"))?;
                    return Err(PlaybackStopped.into());
                }
                PlaybackPhase::Paused => {
                    if !player_paused {
                        mci_command(&format!("pause {ALIAS}"))?;
                        player_paused = true;
                    }
                }
                PlaybackPhase::Playing => {
                    if player_paused {
                        mci_command(&format!("resume {ALIAS}"))?;
                        player_paused = false;
                    } else {
                        elapsed = elapsed.saturating_add(tick);
                    }
                    on_sample(audio.features_at(elapsed, FEATURE_WINDOW));
                }
                PlaybackPhase::Idle | PlaybackPhase::Preparing => {}
            }

            if mci_command(&format!("status {ALIAS} mode"))? == "stopped" {
                return Ok(());
            }
            self.playback.wait_for_phase_change(phase, SAMPLE_INTERVAL);
        }
    }
}

impl AudioPlayer for SystemAudioPlayer {
    fn play(&mut self, audio: &Audio, on_sample: &mut dyn FnMut(AudioFeatures)) -> Result<()> {
        self.write_wav(audio)?;
        #[cfg(target_os = "macos")]
        let status = {
            let mut child = self.start_player()?;
            monitor_playback(&mut child, audio, on_sample, &self.playback)
                .context("could not monitor system audio playback")?
        };
        #[cfg(target_os = "linux")]
        let status = self.play_file(audio, on_sample)?;
        #[cfg(target_os = "windows")]
        self.play_windows(audio, on_sample)?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if !status.success() {
            bail!("system audio playback exited with {status}");
        }
        Ok(())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn monitor_playback(
    child: &mut Child,
    audio: &Audio,
    on_sample: &mut dyn FnMut(AudioFeatures),
    playback: &PlaybackController,
) -> Result<ExitStatus> {
    const SAMPLE_INTERVAL: Duration = Duration::from_millis(40);
    const FEATURE_WINDOW: Duration = Duration::from_millis(80);

    if let Err(error) = playback.begin_playback() {
        stop_child(child);
        return Err(error.into());
    }

    let mut elapsed = Duration::ZERO;
    let mut last_tick = Instant::now();
    let mut process_paused = false;
    loop {
        if let Some(status) = child
            .try_wait()
            .context("could not query system audio playback")?
        {
            return Ok(status);
        }

        let now = Instant::now();
        let tick = now.saturating_duration_since(last_tick);
        last_tick = now;
        let phase = playback.phase();
        match phase {
            PlaybackPhase::Cancelled => {
                stop_child(child);
                return Err(PlaybackStopped.into());
            }
            PlaybackPhase::Paused => {
                if !process_paused {
                    if let Err(error) = set_process_paused(child, true) {
                        stop_child(child);
                        return Err(error);
                    }
                    process_paused = true;
                }
            }
            PlaybackPhase::Playing => {
                if process_paused {
                    if let Err(error) = set_process_paused(child, false) {
                        stop_child(child);
                        return Err(error);
                    }
                    process_paused = false;
                } else {
                    elapsed = elapsed.saturating_add(tick);
                }
                on_sample(audio.features_at(elapsed, FEATURE_WINDOW));
            }
            PlaybackPhase::Idle | PlaybackPhase::Preparing => {}
        }
        playback.wait_for_phase_change(phase, SAMPLE_INTERVAL);
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn set_process_paused(child: &Child, paused: bool) -> Result<()> {
    let signal = if paused { libc::SIGSTOP } else { libc::SIGCONT };
    // SAFETY: `child.id()` is a live process identifier owned by this adapter,
    // and `kill` is called with a non-destructive stop/continue signal.
    let result = unsafe { libc::kill(child.id() as libc::pid_t, signal) };
    if result == -1 {
        return Err(std::io::Error::last_os_error())
            .context("could not change system audio pause state");
    }
    Ok(())
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use std::{sync::mpsc, thread};

    use super::*;

    #[test]
    fn system_player_process_can_pause_resume_and_cancel() {
        let control = PlaybackController::default();
        let mut queue = crate::domain::SpeechQueue::default();
        let id = queue
            .push(crate::domain::SelectedText::new("Audio control test").unwrap())
            .unwrap();
        let token = crate::playback::PlaybackToken::queue_item(id);
        assert!(control.register(token));
        control.begin_session(token).unwrap();
        let mut child = Command::new("/bin/sleep").arg("5").spawn().unwrap();
        let process_id = child.id() as libc::pid_t;
        let (sample_sender, sample_receiver) = mpsc::sync_channel(1);
        let control_thread = control.clone();
        let controller = thread::spawn(move || {
            sample_receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            assert!(control_thread.pause(token));
            let stopped = wait_for_stopped_process(process_id);
            if !stopped {
                control_thread.cancel(token);
                return false;
            }
            // Discard samples queued before SIGSTOP. The next sample must be
            // emitted after the monitor has applied resume to the child.
            sample_receiver.try_iter().for_each(drop);
            assert!(control_thread.resume(token));
            let resumed = sample_receiver.recv_timeout(Duration::from_secs(2)).is_ok();
            assert!(control_thread.cancel(token));
            resumed
        });

        let audio = Audio::new(vec![0.1; 24_000], 24_000);
        let error = monitor_playback(
            &mut child,
            &audio,
            &mut |_| {
                let _ = sample_sender.try_send(());
            },
            &control,
        )
        .unwrap_err();
        let resumed = controller.join().unwrap();
        control.finish_session(token);

        assert!(playback_error_was_stopped(&error));
        assert!(resumed, "system player did not stop and resume sampling");
        assert!(
            child.try_wait().unwrap().is_some(),
            "cancel must reap the child"
        );
    }

    fn wait_for_stopped_process(process_id: libc::pid_t) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let mut status = 0;
            // SAFETY: the caller owns this live child and `status` is a valid
            // writable wait-status pointer. Stop notifications do not reap it.
            let result =
                unsafe { libc::waitpid(process_id, &mut status, libc::WNOHANG | libc::WUNTRACED) };
            if result == process_id && libc::WIFSTOPPED(status) {
                return true;
            }
            if result == -1 {
                return false;
            }
            thread::sleep(Duration::from_millis(1));
        }
        false
    }

    #[test]
    fn cancellation_before_playback_stops_and_reaps_the_system_player() {
        let control = PlaybackController::default();
        let token = crate::playback::PlaybackToken::PREVIEW;
        control.begin_session(token).unwrap();
        assert!(control.cancel(token));
        let mut child = Command::new("/bin/sleep").arg("5").spawn().unwrap();
        let audio = Audio::new(vec![0.1; 24_000], 24_000);

        let error = monitor_playback(
            &mut child,
            &audio,
            &mut |_| panic!("cancelled speech must not emit playback samples"),
            &control,
        )
        .unwrap_err();

        assert!(playback_error_was_stopped(&error));
        assert!(
            child.try_wait().unwrap().is_some(),
            "cancel must reap the child"
        );
    }

    #[test]
    fn completed_system_playback_returns_its_exit_status_without_emitting_samples() {
        let control = PlaybackController::default();
        control
            .begin_session(crate::playback::PlaybackToken::PREVIEW)
            .unwrap();
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        assert!(child.wait().unwrap().success());
        let audio = Audio::new(vec![0.1; 24_000], 24_000);

        let status = monitor_playback(
            &mut child,
            &audio,
            &mut |_| panic!("finished speech must not emit playback samples"),
            &control,
        )
        .unwrap();

        assert!(status.success());
    }

    #[test]
    fn pause_reaches_the_system_player_without_waiting_for_the_visual_sample_tick() {
        let control = PlaybackController::default();
        let mut queue = crate::domain::SpeechQueue::default();
        let id = queue
            .push(crate::domain::SelectedText::new("Immediate pause test").unwrap())
            .unwrap();
        let token = crate::playback::PlaybackToken::queue_item(id);
        assert!(control.register(token));
        control.begin_session(token).unwrap();

        let mut child = Command::new("/bin/sleep").arg("5").spawn().unwrap();
        let process_id = child.id() as libc::pid_t;
        let (sample_sender, sample_receiver) = mpsc::sync_channel(1);
        let controller = control.clone();
        let control_thread = thread::spawn(move || {
            sample_receiver.recv().unwrap();
            let started = Instant::now();
            assert!(controller.pause(token));

            let latency = loop {
                let mut status = 0;
                // SAFETY: `process_id` belongs to the child created for this
                // test and `status` is a valid writable wait-status pointer.
                let result = unsafe {
                    libc::waitpid(process_id, &mut status, libc::WNOHANG | libc::WUNTRACED)
                };
                if result == process_id && libc::WIFSTOPPED(status) {
                    break started.elapsed();
                }
                if started.elapsed() >= Duration::from_millis(250) {
                    break started.elapsed();
                }
                thread::sleep(Duration::from_millis(1));
            };
            assert!(controller.cancel(token));
            latency
        });

        let audio = Audio::new(vec![0.1; 24_000], 24_000);
        let mut sent_sample = false;
        let error = monitor_playback(
            &mut child,
            &audio,
            &mut |_| {
                if !sent_sample {
                    sample_sender.send(()).unwrap();
                    sent_sample = true;
                }
            },
            &control,
        )
        .unwrap_err();
        let pause_latency = control_thread.join().unwrap();
        control.finish_session(token);

        assert!(playback_error_was_stopped(&error));
        assert!(
            pause_latency < Duration::from_millis(20),
            "pause reached the system player after {pause_latency:?}"
        );
    }

    fn playback_error_was_stopped(error: &anyhow::Error) -> bool {
        error
            .chain()
            .any(|cause| cause.downcast_ref::<PlaybackStopped>().is_some())
    }
}

#[cfg(target_os = "windows")]
fn mci_command(command: &str) -> Result<String> {
    use std::{iter, ptr};

    use windows_sys::Win32::Media::Multimedia::{mciGetErrorStringW, mciSendStringW};

    let command: Vec<u16> = command.encode_utf16().chain(iter::once(0)).collect();
    let mut output = vec![0_u16; 128];
    // SAFETY: both UTF-16 buffers are NUL-terminated/appropriately sized and
    // no callback window is used. winmm writes at most `output.len()` units.
    let code = unsafe {
        mciSendStringW(
            command.as_ptr(),
            output.as_mut_ptr(),
            output.len() as u32,
            ptr::null_mut(),
        )
    };
    if code != 0 {
        let mut message = vec![0_u16; 256];
        // SAFETY: `message` is a writable UTF-16 buffer with the advertised size.
        let described =
            unsafe { mciGetErrorStringW(code, message.as_mut_ptr(), message.len() as u32) != 0 };
        let detail = if described {
            String::from_utf16_lossy(&message[..nul_position(&message)])
        } else {
            format!("MCI error {code}")
        };
        bail!("Windows audio control failed: {detail}");
    }
    Ok(String::from_utf16_lossy(&output[..nul_position(&output)])
        .trim()
        .to_owned())
}

#[cfg(target_os = "windows")]
fn nul_position(value: &[u16]) -> usize {
    value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len())
}

#[cfg(target_os = "windows")]
struct MciSession(&'static str);

#[cfg(target_os = "windows")]
impl Drop for MciSession {
    fn drop(&mut self) {
        let _ = mci_command(&format!("close {}", self.0));
    }
}
