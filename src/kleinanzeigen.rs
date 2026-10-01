//! Kleinanzeigen search page scraper.
//!
//! Selectors track the Astro plus Tailwind relaunch using article identifiers and title links.

use {
    reqwest::Client,
    scraper::{ElementRef, Html, Selector},
    thiserror::Error,
};

use crate::ad::Ad;

/// CSS selector for advertisement containers.
const ARTICLE_SELECTOR: &str = "article[data-adid]";

/// CSS selector for the preferred title link inside headings.
const TITLE_LINK_SELECTOR: &str = "h3 a[href^=\"/s-anzeige/\"]";

/// CSS selector for any advertisement link as fallback.
const TITLE_LINK_FALLBACK: &str = "a[href^=\"/s-anzeige/\"]";

/// CSS selector for the preferred image container.
const IMAGE_SELECTOR: &str = "div[data-image-container] img";

/// CSS selector for any image as fallback.
const IMAGE_FALLBACK: &str = "img";

/// Prefix identifying internal advertisement links.
const AD_LINK_PREFIX: &str = "/s-anzeige/";

/// Host prefix used to build absolute advertisement links.
const AD_HOST: &str = "https://www.kleinanzeigen.de";

/// Query suffix requesting the high resolution image variant.
const IMAGE_RULE_SUFFIX: &str = "?rule=$_59.AUTO";

/// Error type for scraping operations.
#[derive(Debug, Error)]
pub enum ScrapeError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Request(String),
    /// Response body could not be read.
    #[error("Response body could not be read: {0}")]
    Body(String),
    /// CSS selector is invalid.
    #[error("CSS selector is invalid: {0}")]
    Selector(String),
}

impl ScrapeError {
    /// Request constructor avoiding variant paths at call sites.
    ///
    /// # Arguments
    ///
    /// * `message` - Underlying error message.
    ///
    /// # Returns
    ///
    /// * `Self` - Request error.
    const fn request(message: String) -> Self {
        Self::Request(message)
    }

    /// Body constructor avoiding variant paths at call sites.
    ///
    /// # Arguments
    ///
    /// * `message` - Underlying error message.
    ///
    /// # Returns
    ///
    /// * `Self` - Body error.
    const fn body(message: String) -> Self {
        Self::Body(message)
    }

    /// Selector constructor avoiding variant paths at call sites.
    ///
    /// # Arguments
    ///
    /// * `message` - Underlying error message.
    ///
    /// # Returns
    ///
    /// * `Self` - Selector error.
    const fn selector(message: String) -> Self {
        Self::Selector(message)
    }
}

/// Scrapes one Kleinanzeigen page for listings.
///
/// # Arguments
///
/// * `client` - HTTP client used for the request.
/// * `url` - Exact page URL to scrape.
///
/// # Returns
///
/// * `Result<Vec<Ad>, ScrapeError>` - Ads found on the page or a descriptive error.
///
/// # Errors
///
/// * `ScrapeError` - Request, body, or selector failed.
pub async fn scrape_page(client: &Client, url: &str) -> Result<Vec<Ad>, ScrapeError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| ScrapeError::request(error.to_string()))?;
    let body = response
        .text()
        .await
        .map_err(|error| ScrapeError::body(error.to_string()))?;
    parse_document(&body)
}

/// Parses a search document into advertisements.
///
/// # Arguments
///
/// * `html` - Raw HTML document text.
///
/// # Returns
///
/// * `Result<Vec<Ad>, ScrapeError>` - Parsed advertisements or a selector error.
fn parse_document(html: &str) -> Result<Vec<Ad>, ScrapeError> {
    let document = Html::parse_document(html);
    let article_selector = parse_selector(ARTICLE_SELECTOR)?;
    Ok(document
        .select(&article_selector)
        .filter_map(article_to_ad)
        .collect())
}

/// Parses a CSS selector string.
///
/// # Arguments
///
/// * `pattern` - Selector pattern text.
///
/// # Returns
///
/// * `Result<Selector, ScrapeError>` - Parsed selector or a descriptive error.
fn parse_selector(pattern: &str) -> Result<Selector, ScrapeError> {
    Selector::parse(pattern).map_err(|error| ScrapeError::selector(error.to_string()))
}

/// Converts one article element into an advertisement.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `Option<Ad>` - Advertisement when identifier and link resolve.
fn article_to_ad(article: ElementRef<'_>) -> Option<Ad> {
    let ad_id = article.value().attr("data-adid")?;
    let href = resolve_href(&article)?;
    if !href.starts_with(AD_LINK_PREFIX) {
        return None;
    }
    Some(Ad {
        id: ad_id.to_owned(),
        title: resolve_title(&article),
        link: format!("{AD_HOST}{href}"),
        image_url: resolve_image(&article),
    })
}

/// Resolves the advertisement link from title anchors or article fallback.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `Option<String>` - Href value when present.
fn resolve_href(article: &ElementRef<'_>) -> Option<String> {
    title_element(article)
        .and_then(|element| element.value().attr("href").map(str::to_owned))
        .or_else(|| article.value().attr("data-href").map(str::to_owned))
}

/// Finds the preferred title element with fallback.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `Option<ElementRef<'_>>` - Title anchor when present.
fn title_element<'a>(article: &'a ElementRef<'a>) -> Option<ElementRef<'a>> {
    let Ok(preferred) = parse_selector(TITLE_LINK_SELECTOR) else {
        return fallback_title(article);
    };
    if let Some(element) = article.select(&preferred).next() {
        return Some(element);
    }
    fallback_title(article)
}

/// Finds the fallback title element.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `Option<ElementRef<'_>>` - Title anchor when present.
fn fallback_title<'a>(article: &'a ElementRef<'a>) -> Option<ElementRef<'a>> {
    let Ok(selector) = parse_selector(TITLE_LINK_FALLBACK) else {
        return None;
    };
    article.select(&selector).next()
}

/// Resolves human readable title text.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `String` - Trimmed title text or an empty string.
fn resolve_title(article: &ElementRef<'_>) -> String {
    title_element(article)
        .map(|element| element.text().collect::<String>().trim().to_owned())
        .filter(|title| !title.is_empty())
        .unwrap_or_default()
}

/// Resolves the best image URL for one article.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `Option<String>` - Upgraded image URL when present.
fn resolve_image(article: &ElementRef<'_>) -> Option<String> {
    let img = image_element(article)?;
    best_source(&img).map(|src| upgrade_image_url(&src))
}

/// Finds the preferred image element with fallback.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `Option<ElementRef<'_>>` - Image element when present.
fn image_element<'a>(article: &'a ElementRef<'a>) -> Option<ElementRef<'a>> {
    let Ok(preferred) = parse_selector(IMAGE_SELECTOR) else {
        return fallback_image(article);
    };
    if let Some(element) = article.select(&preferred).next() {
        return Some(element);
    }
    fallback_image(article)
}

/// Finds the fallback image element.
///
/// # Arguments
///
/// * `article` - Article element reference from the document.
///
/// # Returns
///
/// * `Option<ElementRef<'_>>` - Image element when present.
fn fallback_image<'a>(article: &'a ElementRef<'a>) -> Option<ElementRef<'a>> {
    let Ok(selector) = parse_selector(IMAGE_FALLBACK) else {
        return None;
    };
    article.select(&selector).next()
}

/// Picks the highest resolution candidate from srcset or src.
///
/// # Arguments
///
/// * `img` - Image element reference.
///
/// # Returns
///
/// * `Option<String>` - Raw image source when present.
fn best_source(img: &ElementRef<'_>) -> Option<String> {
    img.value()
        .attr("srcset")
        .and_then(|srcset| {
            srcset
                .split(',')
                .next_back()
                .and_then(|candidate| candidate.split_whitespace().next())
                .map(str::to_owned)
        })
        .or_else(|| img.value().attr("src").map(str::to_owned))
}

/// Upgrades an image URL to the high resolution variant.
///
/// # Arguments
///
/// * `src` - Raw image source URL.
///
/// # Returns
///
/// * `String` - Image URL with the high resolution rule applied.
fn upgrade_image_url(src: &str) -> String {
    src.split('?').next().map_or_else(
        || src.to_owned(),
        |base| format!("{base}{IMAGE_RULE_SUFFIX}"),
    )
}

#[cfg(test)]
mod tests {
    use {
        anyhow::{Result, anyhow, ensure},
        scraper::Selector,
    };

    use crate::kleinanzeigen::{parse_document, parse_selector, upgrade_image_url};

    #[test]
    fn parses_article_selector() -> Result<()> {
        let selector = parse_selector("article[data-adid]");
        ensure!(selector.is_ok(), "article selector must parse.");
        Ok(())
    }

    #[test]
    fn upgrades_image_url() -> Result<()> {
        let upgraded = upgrade_image_url("https://example.com/img.jpg?rule=old");
        ensure!(
            upgraded == "https://example.com/img.jpg?rule=$_59.AUTO",
            "image URL must use the high resolution rule."
        );
        Ok(())
    }

    #[test]
    fn finds_article_identifier() -> Result<()> {
        let ads = parse_document("<article data-adid=\"42\"></article>")
            .map_err(|error| anyhow!("{error}"))?;
        ensure!(ads.is_empty(), "article without link must be skipped.");
        let parsed = Selector::parse("article[data-adid]").map_err(|error| anyhow!("{error:?}"))?;
        ensure!(
            format!("{parsed:?}").contains("article"),
            "selector must parse article."
        );
        Ok(())
    }
}
