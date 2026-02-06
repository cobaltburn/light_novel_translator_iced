use crate::error::Result;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub mod actions;
pub mod app;
pub mod controller;
pub mod error;
pub mod message;
pub mod model;
pub mod view;
pub mod widget;

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::new("light_novel_translator_iced=debug,error"))
        .with(tracing_subscriber::fmt::layer())
        .init();
    log::info!("logging is enabled");

    app::app()
}
