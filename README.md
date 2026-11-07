<div align="center">

# vaegram

[![Crates.io](https://img.shields.io/crates/v/vaegram)](https://crates.io/crates/vaegram)
[![Docs.rs](https://img.shields.io/docsrs/vaegram)](https://docs.rs/vaegram)
[![Downloads](https://img.shields.io/crates/d/vaegram)](https://crates.io/crates/vaegram)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org)
[![License](https://img.shields.io/github/license/rjriajul/vaegram)](https://github.com/rjriajul/vaegram/blob/main/LICENSE)

**Fast, modern and asynchronous Telegram MTProto API framework in Rust for users and bots**

</div>

**vaegram** talks to Telegram's main API directly over [MTProto](https://core.telegram.org/mtproto), written from
scratch in Rust. Run it as a bot, as a full alternative to the Bot API with no HTTP server in between, or as a user
account, a custom Telegram client. You get typed handlers and filters, conversations, media upload and download,
sessions that survive restarts, and a friendly method for most of what Telegram offers: chats, messages, stories,
payments, gifts and business accounts. Anything else is one `invoke` away, with every function of the latest API
layer generated from Telegram's own schema.

```rust
use vaegram::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let app = Client::builder("my_bot", 12345, "0123456789abcdef0123456789abcdef")
        .bot_token("123456:ABC-DEF")
        .start()
        .await?;

    app.on_message(filters::command("start") & filters::private(), |msg| async move {
        msg.answer("Hello from vaegram!").await?;
        Ok(())
    });

    app.run().await
}
```

### Key Features

- **Pure Rust**: its own MTProto 2.0 transport, auth key handshake and encrypted session, with all of Telegram's
  security checks.
- **Up to date**: all functions of the latest layer (233), generated from the TL schema at build time.
- **Users and bots**: phone login with 2FA, QR login, bot tokens, and business connections.
- **Handlers and conversations**: `on_message` and 40-odd other handlers, composable filters, `ask`, `listen` and
  next-step handlers.
- **Reliable updates**: gap recovery after reconnects and restarts, with the update state saved in the session.
- **Pluggable storage**: a session file by default; SQLite, redb, Redis, MongoDB, S3 and Vault behind features.
- **Observable**: an observer sees every request, `ClientEvent` reports what went wrong, and traffic can be
  recorded and replayed in tests.
- **Fast**: crypto by [WarpCrypto](https://github.com/rjriajul/WarpCrypto).

### Installing

```toml
[dependencies]
vaegram = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Requires Rust 1.85+. Prefer a shorter name? `vae = "0.2"` is the same crate: `use vae::Client;`.

You need an `api_id` and `api_hash` from [my.telegram.org](https://my.telegram.org), and for a bot, a token from
[@BotFather](https://t.me/BotFather). Leave out `.bot_token(..)` and `start()` logs in a user account, asking for
the phone number, the code and the 2FA password. More in [examples](examples).

### Sessions

The first run logs in and writes `my_bot.session`; later runs reuse it, with the chats it knows and the updates
missed while offline. Only one client at a time can use a session file. `.in_memory()` keeps nothing on disk,
`.storage_engine(..)` stores it anywhere, and `export_session_string()` puts the login in one line for an env var.

> A session file or string **is** the login: anyone who has it is logged in as that account. Never commit or share
> it.

### Raw API and Errors

```rust
use vaegram::raw::functions;

let config = app.invoke(&functions::help::GetConfig {}).await?;

match app.send_message(chat_id, "hi").await {
    Err(e) if e.is("CHAT_WRITE_FORBIDDEN") => println!("no rights to write there"),
    Err(e) if e.flood_wait().is_some() => println!("slow down"),
    other => { other?; }
}
```

FLOOD_WAITs up to a minute are slept off and retried. Chat ids follow the Bot API: users positive, groups negative,
channels `-100…`; methods also take `"me"`, `"@username"`, a `t.me` link or a `&Chat`, `&User` or `&Message`.

### Testing Offline

`.record("bot.rec")` writes a client's traffic, and `session::Replay` serves it back on localhost, so handlers run
against real Telegram traffic with no network and no credentials. See
[examples/replay_test.rs](examples/replay_test.rs).

### Resources

- [Documentation](https://docs.rs/vaegram)
- [Releases](https://github.com/rjriajul/vaegram/releases)
- [Examples](examples)
- [Issue tracker](https://github.com/rjriajul/vaegram/issues)

### Development

```bash
cargo test --workspace                                         # offline tests
cargo test --features unstable-raw --test live -- --ignored    # live tests against Telegram
cargo bench --bench tl                                         # benchmarks
```

### License

[Apache-2.0](LICENSE). Use it, fork it, change it, ship it. Every copy and fork must keep the [NOTICE](NOTICE) file
crediting vaegram and Riajul, and may not use the vaegram or vae name to promote itself.
