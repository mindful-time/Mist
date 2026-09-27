use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::{Context, bail};
use ashpd::{AppID, Error, desktop::global_shortcuts::*};
use futures_util::StreamExt;

const APP_ID: &str = "dev.akshobhya.SelectToSpeak";
const SHORTCUT_ID: &str = "speak-selection";

pub enum WaylandShortcutMessage {
    Registered(String),
    Activated,
    Error(String),
}

pub fn spawn() -> Receiver<WaylandShortcutMessage> {
    let (sender, receiver) = mpsc::channel();
    let thread_sender = sender.clone();
    let spawn_result = std::thread::Builder::new()
        .name("wayland-global-shortcut".to_owned())
        .spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| runtime.block_on(run(thread_sender.clone())));
            if let Err(error) = result {
                let _ = thread_sender.send(WaylandShortcutMessage::Error(format!(
                    "Wayland shortcut unavailable: {error:#}"
                )));
            }
        });
    if let Err(error) = spawn_result {
        let _ = sender.send(WaylandShortcutMessage::Error(format!(
            "Could not start the Wayland shortcut listener: {error}"
        )));
    }
    receiver
}

async fn run(sender: Sender<WaylandShortcutMessage>) -> anyhow::Result<()> {
    let app_id = AppID::try_from(APP_ID).context("the Linux application ID is invalid")?;

    // Older portal installations may not expose the Registry interface. In
    // that case GlobalShortcuts can still work, so only this absence is soft.
    match ashpd::register_host_app(app_id).await {
        Ok(()) | Err(Error::PortalNotFound(_)) => {}
        Err(error) => return Err(error).context("could not register the Linux application ID"),
    }

    let shortcuts = GlobalShortcuts::new()
        .await
        .context("the desktop does not provide the GlobalShortcuts portal")?;
    let session = shortcuts
        .create_session(Default::default())
        .await
        .context("could not create a global-shortcut session")?;
    let requested = [
        NewShortcut::new(SHORTCUT_ID, "Speak selected text with Kokoro")
            .preferred_trigger("CTRL+space"),
    ];
    let bound = shortcuts
        .bind_shortcuts(&session, &requested, None, BindShortcutsOptions::default())
        .await?
        .response()?;
    let granted = bound
        .shortcuts()
        .iter()
        .find(|shortcut| shortcut.id() == SHORTCUT_ID)
        .context("the desktop did not grant the Speak Selection shortcut")?;
    let trigger = match granted.trigger_description().trim() {
        "" => "Ctrl+Space".to_owned(),
        description => description.to_owned(),
    };

    let mut activations = shortcuts.receive_activated().await?;
    if sender
        .send(WaylandShortcutMessage::Registered(trigger))
        .is_err()
    {
        return Ok(());
    }
    while let Some(event) = activations.next().await {
        if event.shortcut_id() == SHORTCUT_ID
            && sender.send(WaylandShortcutMessage::Activated).is_err()
        {
            return Ok(());
        }
    }
    bail!("the Wayland global-shortcut session ended unexpectedly")
}
