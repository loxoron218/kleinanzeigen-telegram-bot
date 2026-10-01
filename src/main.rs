//! Kleinanzeigen Telegram bot binary entry point.
//!
//! The binary initialises structured logging and delegates scanning to the application layer.

pub mod ad;
pub mod app;
pub mod config;
pub mod kleinanzeigen;
pub mod seen;
pub mod telegram;

use std::io::stderr;

use {
    anyhow::{Context, Result},
    tokio::main,
    tracing::info,
    tracing_appender::rolling::daily,
    tracing_subscriber::{
        EnvFilter, fmt::layer, prelude::__tracing_subscriber_SubscriberExt, registry,
        util::SubscriberInitExt,
    },
};

use crate::{app::run, config::Config};

/// Runs the bot scan once with tracing initialised.
///
/// # Returns
///
/// * `Result<()>` - Success or a descriptive error.
#[main]
async fn main() -> Result<()> {
    init_tracing().context("Failed to initialise tracing.")?;
    let config = Config::from_env().context("Failed to load configuration.")?;
    run(&config).await.context("Bot run failed.")?;
    info!(status = "completed", "Bot run completed.");
    Ok(())
}

/// Initialises tracing with file and stderr layers.
///
/// # Returns
///
/// * `Result<()>` - Success or a descriptive filter error.
fn init_tracing() -> Result<()> {
    let file_appender = daily("logs", "bot.log");
    let file_layer = layer().json().with_writer(file_appender);
    let stderr_layer = layer().with_writer(stderr);
    registry()
        .with(EnvFilter::from_default_env())
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .context("Tracing init failed.")?;
    Ok(())
}
