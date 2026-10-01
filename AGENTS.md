---
name: code_agent
description: Senior Rust developer using modern idiomatic async Rust for kleinanzeigen-telegram-bot
---

# kleinanzeigen-telegram-bot

Async Rust CLI bot that scrapes Kleinanzeigen "zu verschenken" listings and notifies a Telegram
group; designed to run as a systemd timer every 5 minutes.

## Tech stack

- **Concurrency:** tokio (`macros`, `rt-multi-thread`)
- **Data & Persistence:** serde (`derive`), serde_json (`alloc`)
- **Utilities:** anyhow, reqwest (`form`, `rustls`), scraper, tempfile (dev only), thiserror,
  tracing + tracing-subscriber + tracing-appender

## Codebase map (src/)

- `src/main.rs` — binary entry: `init_tracing()`, `Config::from_env()`, `app::run()`
- `src/app.rs` — orchestration: `build_client`, `crawl_all`, `notify_new`, `handle_one`,
  photo-with-text-fallback + 3-attempt retry, `caption_for`
- `src/config.rs` — env config (`TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID` required) with defaults for
  `base_url`, `url_suffix`, `seen_file`, `max_seen` (1000), `first_run_limit` (25), `max_pages`
  (10); `page_url(page)`
- `src/ad.rs` — shared `Ad { id, title, link, image_url }` domain type
- `src/kleinanzeigen.rs` — scraper: `scrape_page`, `parse_document`, title/image resolution with
  fallbacks, `?rule=$_59.AUTO` image upgrade
- `src/seen.rs` — persistence: `load` (empty queue + warn on failure), `save`, `prune`
- `src/telegram.rs` — API client: `send_photo`, `send_text`, shared `interpret_response` /
  `retry_after_from_bytes` / `is_rate_limited`

## Conventions & workflow

- ALWAYS read `CODING_STANDARDS.md` in full before any code-related task — single source of truth
  for style, error handling, concurrency, tracing, docs, testing, and build commands (lint, format,
  test, bench) — no exceptions..
