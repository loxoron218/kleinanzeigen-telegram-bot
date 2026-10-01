//! Telegram Bot API client with shared rate limit handling.
//!
//! Photo and text sending share one response interpreter so clones cannot drift apart.

use {
    reqwest::{Client, Response},
    serde::Deserialize,
    serde_json::from_slice,
    thiserror::Error,
};

use crate::config::Config;

/// Default wait time after rate limiting without a hint.
const DEFAULT_RETRY_AFTER_SECS: u64 = 30;

/// Telegram rate limit error code.
const RATE_LIMIT_CODE: i32 = 429;

/// Telegram API error payload.
#[derive(Debug, Deserialize)]
struct TelegramApiError {
    /// Machine readable error code.
    error_code: Option<i32>,
    /// Additional error parameters.
    parameters: Option<TelegramApiParameters>,
}

/// Additional parameters for Telegram API errors.
#[derive(Debug, Deserialize)]
struct TelegramApiParameters {
    /// Suggested wait time before retrying.
    retry_after: Option<i64>,
}

/// Error type for Telegram operations.
#[derive(Debug, Error)]
pub enum TelegramError {
    /// HTTP transport failed.
    #[error("HTTP transport failed: {0}")]
    Transport(String),
    /// Telegram API returned an error.
    #[error("Telegram API error {status}: {body}")]
    Api {
        /// HTTP status code text.
        status: String,
        /// Response body text.
        body: String,
    },
}

impl TelegramError {
    /// Transport constructor avoiding variant paths at call sites.
    ///
    /// # Arguments
    ///
    /// * `message` - Underlying error message.
    ///
    /// # Returns
    ///
    /// * `Self` - Transport error.
    const fn transport(message: String) -> Self {
        Self::Transport(message)
    }

    /// API constructor avoiding variant paths at call sites.
    ///
    /// # Arguments
    ///
    /// * `status` - HTTP status code text.
    /// * `body` - Response body text.
    ///
    /// # Returns
    ///
    /// * `Self` - API error.
    const fn api(status: String, body: String) -> Self {
        Self::Api { status, body }
    }
}

/// Sends a photo with a caption to the configured chat.
///
/// # Arguments
///
/// * `client` - HTTP client used for the API call.
/// * `config` - Runtime configuration with credentials.
/// * `photo_url` - Public image URL sent as photo.
/// * `caption` - HTML formatted caption text.
///
/// # Returns
///
/// * `Result<Option<u64>, TelegramError>` - Retry hint, none on success.
///
/// # Errors
///
/// * `TelegramError` - Transport or API error occurred.
pub async fn send_photo(
    client: &Client,
    config: &Config,
    photo_url: &str,
    caption: &str,
) -> Result<Option<u64>, TelegramError> {
    let url = format!("https://api.telegram.org/bot{}/sendPhoto", config.bot_token);
    let params = [
        ("chat_id", config.chat_id.as_str()),
        ("photo", photo_url),
        ("caption", caption),
        ("parse_mode", "HTML"),
    ];
    interpret_response(post_form(client, &url, &params).await?).await
}

/// Sends a text message to the configured chat.
///
/// # Arguments
///
/// * `client` - HTTP client used for the API call.
/// * `config` - Runtime configuration with credentials.
/// * `message` - HTML formatted message text.
///
/// # Returns
///
/// * `Result<Option<u64>, TelegramError>` - Retry hint, none on success.
///
/// # Errors
///
/// * `TelegramError` - Transport or API error occurred.
pub async fn send_text(
    client: &Client,
    config: &Config,
    message: &str,
) -> Result<Option<u64>, TelegramError> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        config.bot_token
    );
    let params = [
        ("chat_id", config.chat_id.as_str()),
        ("text", message),
        ("parse_mode", "HTML"),
    ];
    interpret_response(post_form(client, &url, &params).await?).await
}

/// Posts URL encoded form parameters.
///
/// # Arguments
///
/// * `client` - HTTP client used for the API call.
/// * `url` - Fully qualified Telegram endpoint URL.
/// * `params` - Form fields sent as request body.
///
/// # Returns
///
/// * `Result<Response, TelegramError>` - Raw API response.
async fn post_form(
    client: &Client,
    url: &str,
    params: &[(&str, &str)],
) -> Result<Response, TelegramError> {
    client
        .post(url)
        .form(params)
        .send()
        .await
        .map_err(|error| TelegramError::transport(error.to_string()))
}

/// Interprets one Telegram response including rate limits.
///
/// # Arguments
///
/// * `response` - Raw HTTP response from Telegram.
///
/// # Returns
///
/// * `Result<Option<u64>, TelegramError>` - Retry hint, none on success.
async fn interpret_response(response: Response) -> Result<Option<u64>, TelegramError> {
    let status = response.status();
    if status.is_success() {
        return Ok(None);
    }
    let status_text = status.to_string();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| TelegramError::transport(error.to_string()))?;
    if let Some(retry_after) = retry_after_from_bytes(bytes.as_ref()) {
        return Ok(Some(retry_after));
    }
    if is_rate_limited(bytes.as_ref()) {
        return Ok(Some(DEFAULT_RETRY_AFTER_SECS));
    }
    Err(TelegramError::api(
        status_text,
        String::from_utf8_lossy(bytes.as_ref()).into_owned(),
    ))
}

/// Checks whether a body carries a Telegram rate limit code.
///
/// # Arguments
///
/// * `bytes` - Raw response body bytes.
///
/// # Returns
///
/// * `bool` - True when the body signals error code 429.
fn is_rate_limited(bytes: &[u8]) -> bool {
    let Ok(payload) = from_slice::<TelegramApiError>(bytes) else {
        return false;
    };
    payload
        .error_code
        .is_some_and(|code| code == RATE_LIMIT_CODE)
}

/// Extracts the retry hint from a Telegram error body.
///
/// # Arguments
///
/// * `bytes` - Raw response body bytes.
///
/// # Returns
///
/// * `Option<u64>` - Seconds to wait when present.
fn retry_after_from_bytes(bytes: &[u8]) -> Option<u64> {
    let Ok(payload) = from_slice::<TelegramApiError>(bytes) else {
        return None;
    };
    let code = payload.error_code?;
    if code != RATE_LIMIT_CODE {
        return None;
    }
    let params = payload.parameters?;
    let retry_after = params.retry_after?;
    let Ok(value) = u64::try_from(retry_after) else {
        return None;
    };
    if value == 0 {
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};

    use crate::telegram::{is_rate_limited, retry_after_from_bytes};

    #[test]
    fn parses_retry_hint() -> Result<()> {
        let body = br#"{"error_code":429,"parameters":{"retry_after":12}}"#;
        ensure!(
            retry_after_from_bytes(body) == Some(12),
            "rate limit body must expose twelve seconds."
        );
        Ok(())
    }

    #[test]
    fn ignores_other_errors() -> Result<()> {
        let body = br#"{"error_code":400}"#;
        ensure!(
            retry_after_from_bytes(body).is_none(),
            "other errors must not expose a retry hint."
        );
        ensure!(
            !is_rate_limited(body),
            "other errors must not flag rate limiting."
        );
        Ok(())
    }
}
