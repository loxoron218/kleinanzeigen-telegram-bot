//! Application orchestration for crawling and notification.
//!
//! Crawling collects advertisements first while notification handles delivery with retries.

use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use {
    reqwest::{Client, Error as ReqwestError},
    thiserror::Error,
    tokio::time::sleep,
    tracing::{info, warn},
};

use crate::{
    ad::Ad,
    config::{Config, ConfigError},
    kleinanzeigen::{ScrapeError, scrape_page},
    seen::{SeenError, load, prune, save},
    telegram::{TelegramError, send_photo, send_text},
};

/// Browser user agent avoiding bot blocks.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, \
                          like Gecko) Chrome/91.0.4472.124 Safari/537.36";

/// Delay between search page requests.
const CRAWL_DELAY_SECS: u64 = 1;

/// Delay between Telegram notifications.
const NOTIFY_DELAY_SECS: u64 = 2;

/// Maximum delivery attempts per message.
const DELIVERY_ATTEMPTS: [u32; 3] = [0_u32, 1_u32, 2_u32];

/// Error type for application orchestration.
#[derive(Debug, Error)]
pub enum AppError {
    /// Configuration loading failed.
    #[error("Configuration loading failed: {0}")]
    Config(#[from] ConfigError),
    /// Scraping failed.
    #[error("Scraping failed: {0}")]
    Scrape(#[from] ScrapeError),
    /// Telegram delivery failed.
    #[error("Telegram delivery failed: {0}")]
    Telegram(#[from] TelegramError),
    /// Persistence failed.
    #[error("Persistence failed: {0}")]
    Seen(#[from] SeenError),
    /// HTTP client building failed.
    #[error("HTTP client building failed: {0}")]
    Http(#[from] ReqwestError),
}

/// Runs one full scan and notification cycle.
///
/// # Arguments
///
/// * `config` - Runtime configuration with credentials and tuning.
///
/// # Returns
///
/// * `Result<(), AppError>` - Success or a descriptive error.
///
/// # Errors
///
/// * `AppError` - Client building, crawling, or persistence failed.
pub async fn run(config: &Config) -> Result<(), AppError> {
    let client = build_client()?;
    let mut seen_queue = load(&config.seen_file);
    let is_first_run = seen_queue.is_empty();
    info!(count = seen_queue.len(), "Loaded seen advertisements.");
    let seen_set: HashSet<String> = seen_queue.iter().cloned().collect();
    let all_ads = crawl_all(&client, config, &seen_set).await?;
    let (processed, sent) = notify_new(
        &client,
        config,
        &all_ads,
        &mut seen_queue,
        &seen_set,
        is_first_run,
    )
    .await;
    if processed > 0 {
        prune(&mut seen_queue, config.max_seen);
        info!(retained = seen_queue.len(), "Pruned seen advertisements.");
        save(&config.seen_file, &seen_queue)?;
    }
    info!(processed, sent, "Scan completed.");
    Ok(())
}

/// Builds an HTTP client with a browser user agent.
///
/// # Returns
///
/// * `Result<Client, AppError>` - Configured client or a descriptive error.
fn build_client() -> Result<Client, AppError> {
    Ok(Client::builder().user_agent(USER_AGENT).build()?)
}

/// Crawls search pages until empty, seen, or limited.
///
/// # Arguments
///
/// * `client` - HTTP client used for requests.
/// * `config` - Runtime configuration with URLs and limits.
/// * `seen` - Identifiers already notified about.
///
/// # Returns
///
/// * `Result<Vec<Ad>, AppError>` - Collected advertisements in crawl order.
async fn crawl_all(
    client: &Client,
    config: &Config,
    seen: &HashSet<String>,
) -> Result<Vec<Ad>, AppError> {
    let mut collected: Vec<Ad> = Vec::new();
    for page in 1..=config.max_pages {
        let url = config.page_url(page);
        let current = scrape_page(client, &url).await?;
        if current.is_empty() {
            info!(page, "No advertisements found, stopping crawl.");
            break;
        }
        let found_seen = current.iter().any(|ad| seen.contains(&ad.id));
        collected.extend(current);
        if found_seen {
            info!(page, "Seen advertisement found, stopping crawl.");
            break;
        }
        sleep(Duration::from_secs(CRAWL_DELAY_SECS)).await;
    }
    Ok(collected)
}

/// Notifies about new advertisements with first run limiting.
///
/// # Arguments
///
/// * `client` - HTTP client used for Telegram calls.
/// * `config` - Runtime configuration with limits.
/// * `ads` - Collected advertisements in crawl order.
/// * `seen_queue` - Mutable queue receiving successfully sent identifiers.
/// * `seen` - Identifiers already notified about.
/// * `is_first_run` - Whether this is the very first run.
///
/// # Returns
///
/// * `(usize, usize)` - Processed new advertisements and successfully sent count.
async fn notify_new(
    client: &Client,
    config: &Config,
    ads: &[Ad],
    seen_queue: &mut VecDeque<String>,
    seen: &HashSet<String>,
    is_first_run: bool,
) -> (usize, usize) {
    let mut processed: usize = 0;
    let mut sent: usize = 0;
    for ad in ads {
        if is_first_run && sent >= config.first_run_limit {
            break;
        }
        if seen.contains(&ad.id) {
            continue;
        }
        processed = processed.saturating_add(1);
        if handle_one(client, config, ad).await {
            seen_queue.push_back(ad.id.clone());
            sent = sent.saturating_add(1);
        }
        sleep(Duration::from_secs(NOTIFY_DELAY_SECS)).await;
    }
    (processed, sent)
}

/// Handles one advertisement delivery with fallback.
///
/// # Arguments
///
/// * `client` - HTTP client used for Telegram calls.
/// * `config` - Runtime configuration with credentials.
/// * `ad` - Advertisement to notify about.
///
/// # Returns
///
/// * `bool` - True when delivery succeeded.
async fn handle_one(client: &Client, config: &Config, ad: &Ad) -> bool {
    info!(id = %ad.id, title = %ad.title, "New advertisement found.");
    let caption = caption_for(ad);
    if let Some(image_url) = ad.image_url.as_ref() {
        if deliver_photo(client, config, image_url, &caption).await {
            return true;
        }
        warn!(id = %ad.id, "Photo delivery failed, trying text fallback.");
    }
    deliver_text(client, config, &caption).await
}

/// Builds an HTML caption for one advertisement.
///
/// # Arguments
///
/// * `ad` - Advertisement to describe.
///
/// # Returns
///
/// * `String` - HTML caption with title and link.
fn caption_for(ad: &Ad) -> String {
    format!(
        "<b>Neuer kostenloser Artikel gefunden!</b>\n<b>Titel:</b> {}\n<a href='{}'>Anzeige \
         ansehen</a>",
        ad.title, ad.link
    )
}

/// Delivers one photo message with rate limit retries.
///
/// # Arguments
///
/// * `client` - HTTP client used for Telegram calls.
/// * `config` - Runtime configuration with credentials.
/// * `image_url` - Public image URL sent as photo.
/// * `caption` - HTML formatted caption text.
///
/// # Returns
///
/// * `bool` - True when delivery succeeded.
async fn deliver_photo(client: &Client, config: &Config, image_url: &str, caption: &str) -> bool {
    for _ in DELIVERY_ATTEMPTS {
        match send_photo(client, config, image_url, caption).await {
            Ok(None) => return true,
            Ok(Some(retry_after)) => {
                warn!(retry_after, "Photo rate limited, waiting.");
                sleep(Duration::from_secs(retry_after)).await;
            }
            Err(error) => {
                warn!(error = %error, "Photo delivery failed.");
                return false;
            }
        }
    }
    false
}

/// Delivers one text message with rate limit retries.
///
/// # Arguments
///
/// * `client` - HTTP client used for Telegram calls.
/// * `config` - Runtime configuration with credentials.
/// * `message` - HTML formatted message text.
///
/// # Returns
///
/// * `bool` - True when delivery succeeded.
async fn deliver_text(client: &Client, config: &Config, message: &str) -> bool {
    for _ in DELIVERY_ATTEMPTS {
        match send_text(client, config, message).await {
            Ok(None) => return true,
            Ok(Some(retry_after)) => {
                warn!(retry_after, "Text rate limited, waiting.");
                sleep(Duration::from_secs(retry_after)).await;
            }
            Err(error) => {
                warn!(error = %error, "Text delivery failed.");
                return false;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};

    use crate::{ad::Ad, app::caption_for};

    #[test]
    fn builds_caption() -> Result<()> {
        let ad = Ad {
            id: String::from("7"),
            title: String::from("Tisch"),
            link: String::from("https://www.kleinanzeigen.de/s-anzeige/7"),
            image_url: None,
        };
        let caption = caption_for(&ad);
        ensure!(caption.contains("Tisch"), "caption must contain the title.");
        ensure!(
            caption.contains("https://www.kleinanzeigen.de/s-anzeige/7"),
            "caption must contain the link."
        );
        Ok(())
    }
}
