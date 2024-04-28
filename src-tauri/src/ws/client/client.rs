use crate::ws::message::{ChatMessage, MessageContent};

use anyhow::anyhow;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type ClientWSSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

pub struct ChatHandle {
    client_sink: ClientWSSink,
    client_stream_rx: Receiver<ChatMessage>,
    name: String,
}

impl ChatHandle {
    pub async fn new(identity: String, server_url: String) -> anyhow::Result<Self> {
        let ws_stream = match connect_async(format!("ws://{server_url}/ws/{identity}")).await {
            Ok((stream, response)) => {
                tracing::debug!(
                    "Handshake for client has been completed with {:?}",
                    response
                );

                stream
            }
            Err(e) => {
                return Err(anyhow!("WebSocket handshake failed with {e}!"));
            }
        };

        let (sender, receiver) = ws_stream.split();

        let mut mapped_receiver = receiver.map(|ws_msg| {
            if let Err(e) = ws_msg {
                return Err(anyhow!(e));
            }

            let ws_msg = ws_msg.unwrap();
            let decoded_msg = ChatMessage::try_from(ws_msg);

            decoded_msg
        });

        let (tx, rx) = watch::channel(ChatMessage::new("", "", MessageContent::Close()));

        tokio::spawn(async move {
            loop {
                let msg = mapped_receiver.next().await;

                if msg.is_none() {
                    tracing::debug!("got none msg from stream");
                    break;
                }

                let msg = msg.unwrap();

                if msg.is_err() {
                    tracing::error!("got error message from stream: {:?}", msg.err().unwrap());
                    continue;
                }

                if let Err(e) = tx.send(msg.unwrap()) {
                    tracing::debug!(
                        "failed to send chat message into internal watch channel: {:?}",
                        e
                    );
                    break;
                }
            }

            // TODO: we might want to close the connection here
        });

        Ok(Self {
            name: identity,
            client_sink: sender,
            client_stream_rx: rx,
        })
    }

    pub async fn send_text(&mut self, receiver: String, message: String) -> anyhow::Result<()> {
        let msg = ChatMessage::new(&self.name, &receiver, MessageContent::Prompt(message));

        let wsmsg = msg.try_into().unwrap();

        tracing::debug!("sending message: {:?}", &wsmsg);

        self.client_sink.send(wsmsg).await?;

        Ok(())
    }

    pub async fn list_users(&mut self) -> anyhow::Result<()> {
        let msg = ChatMessage::new("", "", MessageContent::GetUsersList);

        let wsmsg = msg.try_into().unwrap();

        tracing::debug!("sending message: {:?}", &wsmsg);

        self.client_sink.send(wsmsg).await?;

        Ok(())
    }

    pub fn get_receiver(&self) -> Receiver<ChatMessage> {
        self.client_stream_rx.clone()
    }

    pub async fn close(&mut self) -> anyhow::Result<()> {
        self.client_sink
            .send(
                ChatMessage::new("", "", MessageContent::Close())
                    .try_into()
                    .unwrap(),
            )
            .await?;

        Ok(())
    }
}

pub async fn init_client(identity: String, server_url: String) -> anyhow::Result<ChatHandle> {
    let chat_handle = ChatHandle::new(identity, server_url).await?;

    Ok(chat_handle)
}
