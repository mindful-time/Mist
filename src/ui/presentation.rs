use ::mist::{AudioFeatures, worker::AppStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MistActivity {
    Idle,
    Busy,
    Speaking,
    Attention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MistPresentation {
    pub(super) activity: MistActivity,
    pub(super) features: AudioFeatures,
    pub(super) requires_panel: bool,
}

#[derive(Debug, Default)]
pub(super) struct MistSmoother {
    energy: f32,
    brightness: f32,
    last_time: Option<f32>,
}

impl MistSmoother {
    pub(super) fn update(&mut self, mut target: MistPresentation, time: f32) -> MistPresentation {
        let target_energy = f32::from(target.features.energy);
        let target_brightness = f32::from(target.features.brightness);
        let Some(last_time) = self.last_time.replace(time) else {
            self.energy = target_energy;
            self.brightness = target_brightness;
            return target;
        };
        let delta = (time - last_time).clamp(0.0, 0.05);
        self.energy = ease(
            self.energy,
            target_energy,
            delta,
            if target_energy > self.energy {
                10.0
            } else {
                4.0
            },
        );
        self.brightness = ease(
            self.brightness,
            target_brightness,
            delta,
            if target_brightness > self.brightness {
                7.0
            } else {
                3.4
            },
        );
        target.features = AudioFeatures {
            energy: self.energy.round().clamp(0.0, 255.0) as u8,
            brightness: self.brightness.round().clamp(0.0, 255.0) as u8,
        };
        target
    }
}

fn ease(current: f32, target: f32, delta: f32, response: f32) -> f32 {
    current + (target - current) * (1.0 - (-response * delta).exp())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PrimaryAction {
    None,
    InstallVoices,
    OpenAccessibility,
}

pub(super) struct StatusCopy {
    pub(super) eyebrow: &'static str,
    pub(super) title: &'static str,
    pub(super) detail: String,
    pub(super) action: PrimaryAction,
}

pub(super) fn mist_for_status(
    status: &AppStatus,
    accessibility_required: bool,
    shell_error: Option<&str>,
) -> MistPresentation {
    if accessibility_required {
        return MistPresentation {
            activity: MistActivity::Attention,
            features: features(18, 20),
            requires_panel: false,
        };
    }
    match status {
        AppStatus::CheckingModel => busy(false),
        AppStatus::MissingModel => MistPresentation {
            activity: MistActivity::Idle,
            features: features(8, 12),
            requires_panel: true,
        },
        AppStatus::Ready if shell_error.is_some() => MistPresentation {
            activity: MistActivity::Attention,
            features: features(18, 20),
            requires_panel: false,
        },
        AppStatus::Ready => MistPresentation {
            activity: MistActivity::Idle,
            features: features(10, 16),
            requires_panel: false,
        },
        AppStatus::Downloading | AppStatus::Loading | AppStatus::Synthesizing { .. } => busy(false),
        AppStatus::Speaking { features, .. } => MistPresentation {
            activity: MistActivity::Speaking,
            features: *features,
            requires_panel: false,
        },
        AppStatus::Error(_) => MistPresentation {
            activity: MistActivity::Attention,
            features: features(24, 28),
            requires_panel: false,
        },
    }
}

fn busy(requires_panel: bool) -> MistPresentation {
    MistPresentation {
        activity: MistActivity::Busy,
        features: features(32, 36),
        requires_panel,
    }
}

const fn features(energy: u8, brightness: u8) -> AudioFeatures {
    AudioFeatures { energy, brightness }
}

pub(super) fn copy_for_status(
    status: &AppStatus,
    _accessibility_required: bool,
    platform_error: Option<&str>,
    tray_error: Option<&str>,
) -> StatusCopy {
    match status {
        AppStatus::CheckingModel => StatusCopy {
            eyebrow: "WAKING UP",
            title: "Forming your mist",
            detail: "Checking the local speech voices…".to_owned(),
            action: PrimaryAction::None,
        },
        AppStatus::MissingModel => StatusCopy {
            eyebrow: "WELCOME",
            title: "Choose the voice in your mist",
            detail: "Pick a character below, then download the speech model once. Every voice runs locally and privately.".to_owned(),
            action: PrimaryAction::InstallVoices,
        },
        AppStatus::Downloading => StatusCopy {
            eyebrow: "LOCAL SETUP",
            title: "Gathering the voices",
            detail: "Downloading the verified speech model and voice pack. This happens only once.".to_owned(),
            action: PrimaryAction::None,
        },
        AppStatus::Loading => StatusCopy {
            eyebrow: "WARMING UP",
            title: "Giving the mist a voice",
            detail: "Loading the speech engine with the selected local inference provider.".to_owned(),
            action: PrimaryAction::None,
        },
        AppStatus::Synthesizing {
            text,
            inference_policy,
        } => StatusCopy {
            eyebrow: "FORMING SPEECH",
            title: "The first words are taking shape",
            detail: format!("“{text}” · {inference_policy}"),
            action: PrimaryAction::None,
        },
        AppStatus::Speaking {
            text,
            inference_policy,
            ..
        } => StatusCopy {
            eyebrow: "SPEAKING",
            title: "The mist is alive",
            detail: format!("“{text}” · local playback · {inference_policy}"),
            action: PrimaryAction::None,
        },
        AppStatus::Error(message) => StatusCopy {
            eyebrow: "NEEDS ATTENTION",
            title: "The mist lost its shape",
            detail: shorten(message, 180),
            action: PrimaryAction::None,
        },
        AppStatus::Ready => {
            let warning = tray_error.or(platform_error);
            if let Some(message) = warning {
                StatusCopy {
                    eyebrow: "READY WITH A LIMITATION",
                    title: "Your voice is ready",
                    detail: shorten(message, 180),
                    action: PrimaryAction::None,
                }
            } else {
                StatusCopy {
                    eyebrow: "READY",
                    title: "Your mist is listening",
                    detail: "Select text anywhere and press Ctrl+Space. Use the menu-bar or tray mist to change voices.".to_owned(),
                    action: PrimaryAction::None,
                }
            }
        }
    }
}

fn shorten(message: &str, limit: usize) -> String {
    let message = message.trim();
    if message.chars().count() <= limit {
        return message.to_owned();
    }
    let mut shortened: String = message.chars().take(limit.saturating_sub(1)).collect();
    shortened.push('…');
    shortened
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_desktop_surface_is_mist_only() {
        let presentation = mist_for_status(&AppStatus::Ready, false, None);
        assert_eq!(presentation.activity, MistActivity::Idle);
        assert!(!presentation.requires_panel);
    }

    #[test]
    fn actual_audio_energy_drives_the_speaking_mist() {
        let presentation = mist_for_status(
            &AppStatus::Speaking {
                text: "Hello".to_owned(),
                inference_policy: "CoreML → CPU".to_owned(),
                features: features(147, 83),
            },
            false,
            None,
        );
        assert_eq!(presentation.activity, MistActivity::Speaking);
        assert_eq!(presentation.features.energy, 147);
    }

    #[test]
    fn speaking_overrides_a_nonfatal_shell_warning() {
        let presentation = mist_for_status(
            &AppStatus::Speaking {
                text: "Hello".to_owned(),
                inference_policy: "CoreML → CPU".to_owned(),
                features: features(96, 120),
            },
            false,
            Some("The preferred shortcut is already in use"),
        );
        assert_eq!(presentation.activity, MistActivity::Speaking);
        assert_eq!(presentation.features.energy, 96);
    }

    #[test]
    fn setup_stays_actionable_but_privacy_permission_does_not_trap_settings_open() {
        assert!(mist_for_status(&AppStatus::MissingModel, false, None).requires_panel);
        assert!(!mist_for_status(&AppStatus::Ready, true, None).requires_panel);
        assert_eq!(
            copy_for_status(&AppStatus::Ready, true, None, None).action,
            PrimaryAction::None
        );
    }

    #[test]
    fn synthesis_is_not_presented_as_audible_playback() {
        let presentation = mist_for_status(
            &AppStatus::Synthesizing {
                text: "A long selection…".to_owned(),
                inference_policy: "CoreML → CPU".to_owned(),
            },
            false,
            None,
        );
        assert_eq!(presentation.activity, MistActivity::Busy);
    }

    #[test]
    fn voice_energy_eases_between_audio_windows() {
        let mut smoother = MistSmoother::default();
        let idle = mist_for_status(&AppStatus::Ready, false, None);
        assert_eq!(smoother.update(idle, 1.0).features.energy, 10);

        let loud = mist_for_status(
            &AppStatus::Speaking {
                text: "Hello".to_owned(),
                inference_policy: "CoreML → CPU".to_owned(),
                features: features(255, 220),
            },
            false,
            None,
        );
        let first_frame = smoother.update(loud, 1.016);

        assert!(first_frame.features.energy > 10);
        assert!(first_frame.features.energy < 80);
        assert!(first_frame.features.brightness < 80);
    }
}
