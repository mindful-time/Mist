use eframe::egui::Color32;
use select_to_speak::worker::AppStatus;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum PanelKind {
    Ready,
    Setup,
    Busy,
    Speaking,
    Error,
}

#[derive(Clone, Copy)]
pub(super) enum Tone {
    Violet,
    Mint,
    Amber,
    Coral,
}

impl Tone {
    pub(super) fn color(self) -> Color32 {
        match self {
            Self::Violet => Color32::from_rgb(139, 145, 255),
            Self::Mint => Color32::from_rgb(77, 216, 181),
            Self::Amber => Color32::from_rgb(244, 183, 93),
            Self::Coral => Color32::from_rgb(248, 116, 112),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum PrimaryAction {
    None,
    DownloadVoice,
    OpenAccessibility,
}

pub(super) struct Presentation {
    pub(super) kind: PanelKind,
    pub(super) tone: Tone,
    pub(super) badge: &'static str,
    pub(super) title: &'static str,
    pub(super) detail: String,
    pub(super) action: PrimaryAction,
}

pub(super) fn for_status(
    status: &AppStatus,
    registration_error: Option<&str>,
    accessibility_required: bool,
) -> Presentation {
    match status {
        AppStatus::CheckingModel => Presentation {
            kind: PanelKind::Busy,
            tone: Tone::Violet,
            badge: "STARTING",
            title: "Getting things ready",
            detail: "Checking your local Kokoro voice…".to_owned(),
            action: PrimaryAction::None,
        },
        AppStatus::MissingModel => Presentation {
            kind: PanelKind::Setup,
            tone: Tone::Violet,
            badge: "SETUP",
            title: "Set up your voice",
            detail: "Download Kokoro once. Speech stays entirely on this device.".to_owned(),
            action: PrimaryAction::DownloadVoice,
        },
        AppStatus::Ready => {
            if accessibility_required {
                permission_presentation()
            } else if let Some(error) = registration_error {
                error_presentation(error)
            } else {
                Presentation {
                    kind: PanelKind::Ready,
                    tone: Tone::Mint,
                    badge: "READY",
                    title: "Ready to speak",
                    detail: "Select text in any app, then press".to_owned(),
                    action: PrimaryAction::None,
                }
            }
        }
        AppStatus::Downloading => Presentation {
            kind: PanelKind::Busy,
            tone: Tone::Violet,
            badge: "DOWNLOADING",
            title: "Downloading Kokoro",
            detail: "One-time setup. Keep this window open.".to_owned(),
            action: PrimaryAction::None,
        },
        AppStatus::Loading => Presentation {
            kind: PanelKind::Busy,
            tone: Tone::Amber,
            badge: "LOADING",
            title: "Warming up the voice",
            detail: "Preparing local speech for the first time…".to_owned(),
            action: PrimaryAction::None,
        },
        AppStatus::Synthesizing {
            text,
            inference_policy,
        } => Presentation {
            kind: PanelKind::Busy,
            tone: Tone::Amber,
            badge: "GENERATING",
            title: "Preparing first audio",
            detail: format!("Preparing a stream · {inference_policy}\n“{text}”"),
            action: PrimaryAction::None,
        },
        AppStatus::Speaking {
            text,
            inference_policy,
        } => Presentation {
            kind: PanelKind::Speaking,
            tone: Tone::Mint,
            badge: "SPEAKING",
            title: "Speaking now",
            detail: format!("“{text}”\nStreaming · {inference_policy}"),
            action: PrimaryAction::None,
        },
        AppStatus::Error(message) => error_presentation(message),
    }
}

fn permission_presentation() -> Presentation {
    Presentation {
        kind: PanelKind::Error,
        tone: Tone::Coral,
        badge: "PERMISSION",
        title: "Permission needed",
        detail: "Enable Select to Speak in System Settings › Privacy & Security › Accessibility."
            .to_owned(),
        action: PrimaryAction::OpenAccessibility,
    }
}

fn error_presentation(message: &str) -> Presentation {
    Presentation {
        kind: PanelKind::Error,
        tone: Tone::Coral,
        badge: "ATTENTION",
        title: "Something went wrong",
        detail: shorten(message, 145),
        action: PrimaryAction::None,
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
    fn worker_error_takes_precedence_over_pending_accessibility_permission() {
        let presentation = for_status(
            &AppStatus::Error("The speech worker stopped.".to_owned()),
            None,
            true,
        );

        assert_eq!(presentation.badge, "ATTENTION");
        assert_eq!(presentation.title, "Something went wrong");
        assert_eq!(presentation.detail, "The speech worker stopped.");
        assert!(matches!(presentation.action, PrimaryAction::None));
    }

    #[test]
    fn synthesis_is_not_presented_as_audible_playback() {
        let presentation = for_status(
            &AppStatus::Synthesizing {
                text: "A long selection…".to_owned(),
                inference_policy: "CoreML → CPU".to_owned(),
            },
            None,
            false,
        );

        assert_eq!(presentation.badge, "GENERATING");
        assert_eq!(presentation.title, "Preparing first audio");
        assert!(presentation.detail.contains("CoreML → CPU"));
    }
}
