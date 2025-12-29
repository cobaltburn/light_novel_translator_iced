use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub mod actions;
pub mod app;
pub mod components;
pub mod controller;
pub mod error;
pub mod message;
pub mod state;
pub mod view;

fn main() -> iced::Result {
    tracing_subscriber::registry()
        .with(EnvFilter::new("light_novel_translator_iced=debug,error"))
        .with(tracing_subscriber::fmt::layer())
        .init();
    log::info!("logging is enabled");

    app::app()
}
