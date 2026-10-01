//! Advertisement domain type scraped from Kleinanzeigen.
//!
//! The type is intentionally small so scraping, notification, and persistence share one shape.

use serde::{Deserialize, Serialize};

/// Single advertisement listing from Kleinanzeigen.
///
/// This struct holds the essential information scraped from the website for each ad.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ad {
    /// Unique identifier for the ad.
    pub id: String,
    /// Title of the ad listing.
    pub title: String,
    /// Full URL to the ad page.
    pub link: String,
    /// URL of the main image when available.
    pub image_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use {
        anyhow::{Result, ensure},
        serde_json::{from_str, to_string},
    };

    use crate::ad::Ad;

    #[test]
    fn round_trip() -> Result<()> {
        let ad = Ad {
            id: String::from("1"),
            title: String::from("Stuhl"),
            link: String::from("https://www.kleinanzeigen.de/s-anzeige/1"),
            image_url: None,
        };
        let encoded = to_string(&ad)?;
        let decoded: Ad = from_str(&encoded)?;
        ensure!(decoded == ad, "JSON round trip must preserve the ad.");
        Ok(())
    }
}
