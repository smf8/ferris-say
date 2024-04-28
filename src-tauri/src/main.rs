// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod command;
mod settings;
mod ws;

use crate::ws::client;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

use tauri::{
    App, AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem, SystemTraySubmenu, Window,
};
use tokio::{select, time};

use crate::ws::message::MessageContent;
use command::Command;
use tokio::sync::mpsc::{Receiver, UnboundedReceiver, UnboundedSender};
use tokio::sync::{mpsc, Mutex};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_default())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let tray = SystemTray::new().with_id("main");

    let (tx, rx) = mpsc::unbounded_channel::<Command>();
    let (system_tray_tx, command_handler_tx) = (tx.clone(), tx.clone());

    let app = tauri::Builder::default()
        .system_tray(tray)
        .manage(command::CommandState::new(command_handler_tx))
        .invoke_handler(tauri::generate_handler![
            command::send_message,
            command::save_settings
        ])
        .on_system_tray_event(tray_menu_handler(system_tray_tx))
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                event.window().hide().unwrap();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // only on MacOS to stop it from being displayed in cmd+tab list
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if let Ok(matches) = app.get_cli_matches() {
                let sub_command = matches.subcommand;

                if let Some(sub_command) = sub_command {
                    let args = sub_command.matches.args;
                    if sub_command.name.as_str() == "server" {
                        let port = args.get("port").unwrap();

                        if let Value::String(port) = port.value.clone() {
                            tauri::async_runtime::spawn(async move {
                                let _server = ws::server::server::server_init(&port).await;
                            });

                            tauri::async_runtime::block_on(async {
                                tokio::signal::ctrl_c().await.unwrap();

                                app.handle().exit(1);
                            });
                        } else {
                            tracing::error!("invalid port provided: {}", port.value);

                            app.app_handle().exit(1);
                        }
                    }
                }
            } else {
                tracing::error!(
                    "invalid usage of cli: {}",
                    app.get_cli_matches().err().unwrap()
                );

                app.app_handle().exit(1);
            }

            init_client(app, rx);

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            println!("got exit signal");
            api.prevent_exit();
        }
    });

    Ok(())
}

fn tray_menu_handler(command_tx: UnboundedSender<Command>) -> impl Fn(&AppHandle, SystemTrayEvent) {
    move |app: &AppHandle, event: SystemTrayEvent| {
        if let tauri::SystemTrayEvent::MenuItemClick { id, .. } = event {
            if id == "quit" {
                app.exit(0);
            } else if id == "refresh" {
                if let Err(e) = command_tx.send(Command::ListUsers) {
                    tracing::error!("failed to send list command: {}", e);
                }
            } else if id == "send" {
                let window = app.get_window("main").unwrap();

                window.show().unwrap();
                window.emit_all("send", "").unwrap();
            } else if id == "hide" {
                let main_window = app.get_window("main").unwrap();

                main_window.hide().unwrap();
            }
        }
    }
}

fn init_client(app: &mut App, command_rx: UnboundedReceiver<Command>) {
    let main_window = app.get_window("main").unwrap();
    let init_window = app.get_window("init-config").unwrap();

    init_window.hide().unwrap();

    let config = settings::Settings::from_file("x-ferris-say.json");
    if let Err(e) = config {
        tracing::error!("failed to read config: {}", e);
        main_window.hide().unwrap();
        init_window.show().unwrap();
    } else if let Ok(config) = config {
        if config.username.is_empty() || config.server.is_empty() {
            tracing::error!("empty config, loading init window");

            main_window.hide().unwrap();
            init_window.show().unwrap();
        } else {
            spawn_tokio_ws(config.username, config.server, main_window, app, command_rx);
        }
    }

    app.tray_handle()
        .set_menu(init_menu_items(&vec![]))
        .unwrap();
}

fn spawn_tokio_ws(
    username: String,
    server: String,
    window: Window,
    app: &mut App,
    command_chan: UnboundedReceiver<Command>,
) {
    let username = Arc::new(username);
    let server = Arc::new(server);
    let window = Arc::new(window);
    let command_chan = Arc::new(Mutex::new(command_chan));

    let app_handle = Arc::new(app.app_handle());
    let tray_handle = Arc::new(app.tray_handle());

    let _handle = tauri::async_runtime::spawn(async move {
        let mut retry_wait = time::interval(Duration::from_secs(5));
        loop {
            let app_handle = Arc::clone(&app_handle);
            let tray_handle = Arc::clone(&tray_handle);

            retry_wait.tick().await;

            let ws_chat_handle =
                client::client::init_client(username.to_string(), server.to_string()).await;

            if let Err(e) = ws_chat_handle {
                tracing::error!("failed to initialize websocket: {:?}", e);

                continue;
            }
            let ws_chat_handle = Arc::new(Mutex::new(ws_chat_handle.unwrap()));

            let signal_chat_handle = Arc::clone(&ws_chat_handle);
            tauri::async_runtime::spawn(async move {
                tokio::signal::ctrl_c().await.unwrap();

                let close_result = signal_chat_handle.lock().await.close().await;

                tracing::info!("received ctrl+c closing connection: {:?}", close_result);

                app_handle.exit(1);
            });

            let message_chat_handle = ws_chat_handle.lock().await;
            let mut message_receiver = message_chat_handle.get_receiver();
            // drop the MutexGuard to unlock it
            drop(message_chat_handle);

            let mut refresh_interval = time::interval(Duration::from_secs(5));

            loop {
                let mut command_chan = command_chan.lock().await;
                select! {
                    _ = refresh_interval.tick() => {
                        // retrieve list of online users for the first time
                        if let Err(e) = ws_chat_handle.lock().await.list_users().await {
                            tracing::error!("failed to send list users command: {}", e);
                        }
                    }

                    received_command = command_chan.recv() => {
                        if received_command.is_none(){
                            tracing::error!("got non message from command channel");
                            break;
                        }

                        let received_command = received_command.unwrap();

                        tracing::debug!("received command: {:?}", &received_command);

                        match received_command {
                            Command::ListUsers => {
                                if let Err(e) = ws_chat_handle.lock().await.list_users().await {
                                    tracing::error!("failed to send list users command: {}", e);
                                }
                            }

                            Command::SendPrompt(receiver, text) => {
                                if let Err(e) = ws_chat_handle.lock().await
                                .send_text(receiver, text).await{
                                    tracing::error!("failed to send text: {}", e);
                                }
                            }

                            _ => {}
                        }
                    }

                    received_message = message_receiver.changed() => {
                        if let Err(e) = received_message {
                            tracing::warn!(
                                "message receiver returned error (sender probably dropped): {:?}",
                                e
                            );
                            break;
                        }

                        {
                            let msg = message_receiver.borrow_and_update();

                            let msg = &*msg;

                            // handle received message
                            {
                                match &msg.content {
                                    MessageContent::ListUsers(list) => {
                                        tray_handle.set_menu(init_menu_items(list)).unwrap();
                                        window.emit_all("online_users", list).unwrap();
                                    }
                                    MessageContent::Prompt(text) => {
                                        window.show().unwrap();
                                        window.emit_all("chat_message", text).unwrap();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

fn init_menu_items(online_users: &Vec<String>) -> SystemTrayMenu {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let refresh = CustomMenuItem::new("refresh".to_string(), "Refresh online users");
    let send = CustomMenuItem::new("send".to_string(), "Send a message");
    let hide = CustomMenuItem::new("hide".to_string(), "Hide");

    let mut online_users_menu_item = SystemTrayMenu::new();

    for user in online_users {
        let user_menu_item = CustomMenuItem::new(user, user);

        online_users_menu_item = online_users_menu_item.add_item(user_menu_item);
    }

    let online_users_sub_menu = SystemTraySubmenu::new("Online Users", online_users_menu_item);

    SystemTrayMenu::new()
        .add_item(quit)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(send)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(hide)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(refresh)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_submenu(online_users_sub_menu)
}
