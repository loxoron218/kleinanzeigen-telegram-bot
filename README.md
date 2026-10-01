# kleinanzeigen-telegram-bot

Async Rust bot that scrapes Kleinanzeigen "zu verschenken" listings and sends new ads to a Telegram group. Designed to run as a `systemd` timer every 5 minutes.

## How it works

- Crawls search pages (`max_pages`, stops on empty page or already-seen ad, 1s delay between pages).
- Sends each new ad as a Telegram photo with caption, falling back to text (2s delay, 3 attempts with `429` retry handling).
- Tracks notified ads in `seen_ads.json` (capped at `max_seen`). First run is capped at `first_run_limit`.
- Logs to `logs/bot.log` (daily JSON) plus stderr via `RUST_LOG`.

## Requirements

- Rust toolchain (cargo)
- Telegram bot token + group chat ID (via [@BotFather](https://t.me/BotFather), add bot as group admin, get chat ID via `https://api.telegram.org/bot<TOKEN>/getUpdates`)

## Configuration

All via environment; only the two secrets are required.

| Variable | Required | Default |
|---|---|---|
| `TELEGRAM_BOT_TOKEN` | yes | - |
| `TELEGRAM_CHAT_ID` | yes | - |
| `RUST_LOG` | no | `info` (set e.g. to `debug`) |

Tuning defaults (in `src/config.rs`): `base_url=https://www.kleinanzeigen.de/s-zu-verschenken-tauschen`, `url_suffix=/04105/c272l4257r10`, `seen_file=seen_ads.json`, `max_seen=1000`, `first_run_limit=25`, `max_pages=10`.

## Run

```bash
git clone https://github.com/loxoron218/kleinanzeigen-telegram-bot
cd kleinanzeigen-telegram-bot
cargo build --release
TELEGRAM_BOT_TOKEN=... TELEGRAM_CHAT_ID=... ./target/release/kleinanzeigen-telegram-bot
```

## systemd timer

`kleinanzeigen.service`:

```ini
[Unit]
Description=Kleinanzeigen Telegram Bot

[Service]
Type=oneshot
User=user
WorkingDirectory=/home/user/.local/share/kleinanzeigen-telegram-bot
Environment=TELEGRAM_BOT_TOKEN=...
Environment=TELEGRAM_CHAT_ID=...
ExecStart=/home/user/.local/share/kleinanzeigen-telegram-bot/target/release/kleinanzeigen-telegram-bot
```

`kleinanzeigen.timer`:

```ini
[Unit]
Description=Run Kleinanzeigen Telegram Bot every 5 minutes

[Timer]
OnBootSec=1min
OnUnitActiveSec=5min

[Install]
WantedBy=timers.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now kleinanzeigen.timer
systemctl status kleinanzeigen.service
journalctl -u kleinanzeigen.service -f
```

## Development

```bash
cargo test
cargo clippy --all-targets --all-features
cargo fmt
```

Layout: `src/main.rs` (entry + tracing), `src/app.rs` (crawl/notify), `src/config.rs` (env), `src/kleinanzeigen.rs` (scraper), `src/telegram.rs` (API), `src/seen.rs` (persistence), `src/ad.rs` (type).

License: GPL-3.0-or-later.
