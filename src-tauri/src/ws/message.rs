use anyhow::anyhow;
use axum::extract::ws::Message as AxumMessage;

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub from: String,
    pub to: String,
    pub content: MessageContent,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageContent {
    Close(),
    Prompt(String),
    GetUsersList,
    ListUsers(Vec<String>),
    Error(ChatError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ChatError {
    UserNotOnline,
}

impl ChatMessage {
    pub fn new(from: &str, to: &str, content: MessageContent) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            content,
        }
    }
}

impl FromStr for ChatMessage {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let msg: ChatMessage = serde_json::from_str(s)?;

        Ok(msg)
    }
}

impl TryInto<Message> for ChatMessage {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Message, Self::Error> {
        match &self.content {
            _ => {
                let json_str = serde_json::to_string(&self)?;

                Ok(Message::Text(json_str))
            }

            MessageContent::Close() => Ok(Message::Close(None)),
        }
    }
}

impl TryFrom<Message> for ChatMessage {
    type Error = anyhow::Error;

    fn try_from(value: Message) -> Result<Self, Self::Error> {
        if let Message::Text(text_msg) = value {
            ChatMessage::from_str(&text_msg)
                .map_err(|e| anyhow!("parse '{}' failed: {:?}", &text_msg, e))
        } else if let Message::Close(_) = value {
            Ok(ChatMessage::new("", "", MessageContent::Close()))
        } else {
            Err(anyhow!("got invalid message type: {:?}", value))
        }
    }
}

impl TryFrom<AxumMessage> for ChatMessage {
    type Error = anyhow::Error;

    fn try_from(value: AxumMessage) -> Result<Self, Self::Error> {
        if let AxumMessage::Text(text_msg) = value {
            ChatMessage::from_str(&text_msg)
                .map_err(|e| anyhow!("parse '{}' failed: {:?}", &text_msg, e))
        } else if let AxumMessage::Close(_) = value {
            Ok(ChatMessage::new("", "", MessageContent::Close()))
        } else {
            Err(anyhow!("got invalid message type: {:?}", value))
        }
    }
}
