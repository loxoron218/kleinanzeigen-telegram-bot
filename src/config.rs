//! Application configuration loaded from the environment.
//!
//! Configuration centralises all tunable values so call sites stay free of hard coded secrets.

use std::env::var;

use thiserror::Error;

/// Default Kleinanzeigen search base URL.
const DEFAULT_BASE_URL: &str = "https://www.kleinanzeigen.de/s-zu-verschenken-tauschen";

/// Default Kleinanzeigen search suffix.
const DEFAULT_URL_SUFFIX: &str = "/04105/c272l4257r10";

/// Default persistence file for seen advertisements.
const DEFAULT_SEEN_FILE: &str = "seen_ads.json";

/// Default cap for retained seen identifiers.
const DEFAULT_MAX_SEEN: usize = 1000;

/// Default cap for notifications on the first run.
const DEFAULT_FIRST_RUN_LIMIT: usize = 25;

/// Default cap for crawled pages per run.
const DEFAULT_MAX_PAGES: u32 = 10;

/// Runtime configuration for the bot.
///
/// All secrets come from the environment while scan tuning keeps safe defaults.
#[derive(Debug, Clone)]
pub struct Config {
    /// Telegram bot token used for API authentication.
    pub bot_token: String,
    /// Telegram chat identifier that receives notifications.
    pub chat_id: String,
    /// Base search URL without page suffix.
    pub base_url: String,
    /// Suffix appended to the base search URL.
    pub url_suffix: String,
    /// Path of the JSON file storing seen advertisement identifiers.
    pub seen_file: String,
    /// Maximum number of seen identifiers retained.
    pub max_seen: usize,
    /// Maximum notifications sent on the very first run.
    pub first_run_limit: usize,
    /// Maximum search result pages crawled per run.
    pub max_pages: u32,
}

impl Config {
    /// Loads configuration from the environment.
    ///
    /// # Returns
    ///
    /// * `Result<Self, ConfigError>` - Loaded configuration or a descriptive error.
    ///
    /// # Errors
    ///
    /// * `ConfigError` - Missing required environment variable.
    pub fn from_env() -> Result<Self, ConfigError> {
        let Ok(bot_token) = var("TELEGRAM_BOT_TOKEN") else {
            return Err(ConfigError::missing_token());
        };
        let Ok(chat_id) = var("TELEGRAM_CHAT_ID") else {
            return Err(ConfigError::missing_chat());
        };
        Ok(Self {
            bot_token,
            chat_id,
            base_url: DEFAULT_BASE_URL.to_owned(),
            url_suffix: DEFAULT_URL_SUFFIX.to_owned(),
            seen_file: DEFAULT_SEEN_FILE.to_owned(),
            max_seen: DEFAULT_MAX_SEEN,
            first_run_limit: DEFAULT_FIRST_RUN_LIMIT,
            max_pages: DEFAULT_MAX_PAGES,
        })
    }

    /// Builds the search URL for the given page number.
    ///
    /// # Arguments
    ///
    /// * `page` - One-based page number starting at one.
    ///
    /// # Returns
    ///
    /// * `String` - Fully qualified search URL for the page.
    #[must_use]
    pub fn page_url(&self, page: u32) -> String {
        if page <= 1 {
            format!("{}{}", self.base_url, self.url_suffix)
        } else {
            format!("{}/seite:{page}{}", self.base_url, self.url_suffix)
        }
    }
}

/// Error type for configuration loading.
#[derive(Debug, Clone, Copy, Error)]
pub enum ConfigError {
    /// Missing bot token environment value.
    #[error("Missing TELEGRAM_BOT_TOKEN.")]
    MissingToken,
    /// Missing chat identifier environment value.
    #[error("Missing TELEGRAM_CHAT_ID.")]
    MissingChat,
}

impl ConfigError {
    /// Missing token constructor avoiding variant paths at call sites.
    ///
    /// # Returns
    ///
    /// * `Self` - Missing token error.
    const fn missing_token() -> Self {
        Self::MissingToken
    }

    /// Missing chat constructor avoiding variant paths at call sites.
    ///
    /// # Returns
    ///
    /// * `Self` - Missing chat error.
    const fn missing_chat() -> Self {
        Self::MissingChat
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};

    use crate::config::Config;

    #[test]
    fn first_page_has_no_segment() -> Result<()> {
        let config = test_config();
        ensure!(
            config.page_url(1)
                == "https://www.kleinanzeigen.de/s-zu-verschenken-tauschen/04105/c272l4257r10",
            "first page URL must omit the page segment."
        );
        Ok(())
    }

    #[test]
    fn later_page_includes_segment() -> Result<()> {
        let config = test_config();
        ensure!(
            config.page_url(2).contains("/seite:2"),
            "later page URL must include the page segment."
        );
        Ok(())
    }

    fn test_config() -> Config {
        Config {
            bot_token: String::from("token"),
            chat_id: String::from("chat"),
            base_url: String::from("https://www.kleinanzeigen.de/s-zu-verschenken-tauschen"),
            url_suffix: String::from("/04105/c272l4257r10"),
            seen_file: String::from("seen_ads.json"),
            max_seen: 1000,
            first_run_limit: 25,
            max_pages: 10,
        }
    }
}
