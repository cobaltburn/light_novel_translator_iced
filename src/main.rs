use crate::error::Result;
use std::{fs::File, sync::Mutex};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod chapter;
mod dialogs;
mod epub;
mod error;
mod llm;
mod page;
mod screens;
mod server;
mod text;
mod widget;

const LOG_FILE: &str = "light_novel_translator.log";

fn main() -> Result<()> {
    let log_file = File::options().create(true).append(true).open(LOG_FILE)?;

    tracing_subscriber::registry()
        .with(EnvFilter::new(
            "light_novel_translator_iced=debug,reqwest_retry=warn,error",
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(Mutex::new(log_file)),
        )
        .init();
    log::info!("logging is enabled");

    app::run()
}
