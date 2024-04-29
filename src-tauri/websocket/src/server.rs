use crate::message::{ChatError, ChatMessage, MessageContent};
use axum::extract::ws::Message::Text;
use axum::extract::{ConnectInfo, Path, State};
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    http,
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

const SERVRE_IDENTITY: &str = "__SERVER__";

struct Group {
    user_sinks: RwLock<HashMap<String, Sender<ChatMessage>>>,
}

pub async fn server_init(port: &str) -> anyhow::Result<()> {
    let group = Group {
        user_sinks: RwLock::new(HashMap::new()),
    };

    let group_state = Arc::new(group);

    let app = Router::new()
        .route("/ws/:user_name", get(handler))
        .with_state(group_state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    tracing::info!("started listening on 0.0.0.0:{}", port);
    
    axum::serve::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    Ok(())
}

async fn handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(user_name): Path<String>,
    ws: WebSocketUpgrade,
    State(group_state): State<Arc<Group>>,
) -> Response {
    // separate block to drop group_state lock
    {
        let group_state = Arc::clone(&group_state);
        let usernames = group_state.user_sinks.read().await;
        if usernames.contains_key(&user_name) {
            return Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body("username already exists".into())
                .unwrap();
        }
    }

    let username = user_name.clone();
    let resp = ws.on_upgrade(move |ws| handle_socket(ws, username, group_state));

    tracing::info!("user {user_name} connected: {}", addr);

    resp
}

// connection scenario: after establishing websocket connection
async fn handle_socket(socket: WebSocket, user_name: String, group_state: Arc<Group>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    let mut sinks = group_state.user_sinks.write().await;
    sinks.insert(user_name.clone(), tx.clone());

    {
        let tx = tx.clone();
        let group_state_cloned = group_state.clone();

        tokio::spawn(async move {
            loop {
                select! {
                    msg = receiver.next() => {
                        if msg.is_none() {
                            tracing::error!("got none from stream");

                            group_state_cloned.user_sinks.write().await
                            .remove(&user_name);

                            break;
                        }

                        let msg = msg.unwrap();

                        if let Err(e) = msg {
                            tracing::error!("found error msg: {:?}", e);

                            group_state_cloned.user_sinks.write().await
                            .remove(&user_name);

                            break;
                        }

                        let msg = msg.unwrap();

                        let chat_message = ChatMessage::try_from(msg);

                        if let Err(e) = chat_message {
                            tracing::error!("failed to parse message: {:?}", e);
                            continue;
                        }
                        let chat_message = chat_message.unwrap();

                        let user_sinks = group_state_cloned.user_sinks.read().await;

                        match chat_message.content{
                            MessageContent::Prompt(_) => {
                                let target_user_tx = user_sinks.get(&chat_message.to);

                                if target_user_tx.is_none() &&
                                        tx.send(ChatMessage::new(SERVRE_IDENTITY,
                                                &user_name,
                                                MessageContent::Error(ChatError::UserNotOnline))).await.is_err() {
                                    tracing::info!("client disconnected");
                                    return;
                                }

                                    tracing::debug!("sent");
                                if target_user_tx.unwrap().send(chat_message).await.is_err() {
                                    tracing::info!("client disconnected");
                                    return;
                                }
                            },

                            MessageContent::GetUsersList => {
                                let target_user_tx = user_sinks.get(&user_name);
                                let online_users = list_online_users(group_state_cloned.clone()).await;
                                let resp = ChatMessage::new(SERVRE_IDENTITY, &user_name, MessageContent::ListUsers(online_users));

                                if target_user_tx.unwrap().send(resp).await.is_err() {
                                    tracing::info!("client disconnected");
                                    return;
                                }
                            }

                            _ => {}
                        }
                    }

                    msg = rx.recv() => {
                        if let Some(msg) = msg {
                            if sender.send(Text(serde_json::to_string(&msg).unwrap())).await.is_err() {
                                // client disconnected
                                return;
                            }
                        }
                    }
                }
            }
        });
    }
}

async fn list_online_users(group_state: Arc<Group>) -> Vec<String> {
    let users = group_state.user_sinks.read().await;
    let mut result = Vec::with_capacity(users.capacity());

    for user in users.keys() {
        result.push(user.clone());
    }

    drop(users);

    result
}
