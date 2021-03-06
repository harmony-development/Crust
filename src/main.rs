#![windows_subsystem = "windows"]

use client::content::ContentStore;
use ui::{screen::ScreenManager, style::DEF_SIZE};

use iced::{Application, Settings};
use tracing_subscriber::EnvFilter;

pub mod client;
pub mod ui;

#[tokio::main]
async fn main() {
    // Create the content store
    let content_store = ContentStore::default();
    content_store.create_req_dirs().unwrap();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
            .unwrap_or_else( |_|
                EnvFilter::from("info,wgpu_core=off,iced_wgpu=off,gfx_memory=off,gfx_descriptor=off,gfx_backend_vulkan=off")
            )
        )
        .pretty()
        .init();

    let mut settings = Settings::with_flags(content_store);
    settings.window.size = (1280, 720);
    settings.antialiasing = false;
    settings.default_font = Some(include_bytes!("NotoSans-Regular.ttf"));
    settings.default_text_size = DEF_SIZE;

    ScreenManager::run(settings).unwrap();
}
