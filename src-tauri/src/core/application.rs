use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tokio::net::TcpListener;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};

use crate::module::{self, Module};
use crate::command::screen_shotter_cmd::try_get_screen_img;

pub fn handle_global_hotkey_event(_app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        let mut rotor_app = INSTANCE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for module in rotor_app.modules.values_mut() {
            if let Some(module_shortcut) = module.get_shortcut() {
                if module_shortcut == *shortcut {
                    module.run().unwrap_or_else(|e| {
                        let flag = module.flag();
                        log::error!("Module {flag} run error: {e}")
                    });
                }
            }
        }
    }
}

pub struct Application {
    pub app: Option<AppHandle>,
    pub modules: HashMap<String, Box<dyn module::Module + Send + 'static>>,
    pub ws_port: u16,
}

impl Application {
    fn new() -> Application {
        let mut modules = HashMap::new();

        for module in module::get_all_modules() {
            modules.insert(module.flag().to_string(), module);
        }

        Application { app: None, modules, ws_port: 9001 }
    }

    pub fn global() -> &'static Mutex<Application> {
        &INSTANCE
    }

    pub fn init(&mut self, app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        for module in self.modules.values_mut() {
            module.init(&app).unwrap_or_else(|e| {
                let flag = module.flag();
                log::error!("Module {flag} init error: {e}");
            });
            if let Some(shortcut) = module.get_shortcut() {
                app.global_shortcut().register(shortcut)?;
            }
        }
        self.app = Some(app);

        tauri::async_runtime::spawn(async move {
            Application::run_data_server().await;
        });

        Ok(())
    }

    async fn run_data_server() {
        // Try ports 9001-10000 until one is available
        let port = 48137;
        let listener = TcpListener::bind(format!("localhost:{}", port))
            .await
            .expect("Failed to bind port 48137");
        
        // Update the port in a separate scope to ensure MutexGuard is dropped before await
        {
            let mut rotor_app = Application::global()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            rotor_app.ws_port = port;
        }
        
        while let Ok((stream, _)) = listener.accept().await {
            tauri::async_runtime::spawn(async move {
                let ws_stream = match accept_async(stream).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        log::error!("Error during the websocket handshake occurred: {}", e);
                        return;
                    }
                };

                let (mut write, mut read) = ws_stream.split();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(msg) => {
                            let label = msg.to_string();
                            let image = try_get_screen_img(&label).await;
                            write.send(Message::Binary(image.unwrap_or_default().to_vec().into())).await.expect("Failed to send message");
                        }
                        Err(e) => {
                            log::error!("Error processing message: {}", e);
                            break;
                        }
                    }
                }
            });
        }
    }

    pub fn get_module(&mut self, flag: &str) -> Option<&mut Box<dyn Module + Send + 'static>> {
        self.modules.get_mut(flag)
    }
}

static INSTANCE: LazyLock<Mutex<Application>> = LazyLock::new(|| Mutex::new(Application::new()));
