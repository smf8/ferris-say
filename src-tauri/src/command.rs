use crate::settings;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum Command {
    ListUsers,
    Reconnect,
    SendPrompt(String, String),
    SaveSettings(String, String),
}

#[derive(Debug)]
pub struct CommandState {
    tx: UnboundedSender<Command>,
}

impl CommandState {
    pub fn new(tx: UnboundedSender<Command>) -> Self {
        Self { tx }
    }
}

#[tauri::command]
pub async fn save_settings(username: String, server: String) -> Result<(), bool> {
    let settings = settings::Settings::new(username, server);
    let res = settings.save_to_file("x-ferris-say.json");

    if res.is_err() {
        tracing::error!("failed to save settings: {}", res.err().unwrap());
        Err(true)
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn send_message(
    receiver: String,
    text: String,
    state: tauri::State<'_, CommandState>,
) -> Result<(), bool> {
    tracing::debug!("invoking send_message to {}", &receiver);

    let cmd = Command::SendPrompt(receiver, text);
    let res = state.tx.send(cmd);

    if res.is_err() {
        Err(true)
    } else {
        Ok(())
    }
}
