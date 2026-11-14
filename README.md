<div align="center">

# vaegram

**A Telegram MTProto client in Rust, for bots and user accounts**

[Documentation](https://docs.rs/vaegram) • [Examples](examples) • [Releases](https://github.com/rjriajul/vaegram/releases)

[![Crates.io](https://img.shields.io/crates/v/vaegram)](https://crates.io/crates/vaegram)
[![Docs.rs](https://img.shields.io/docsrs/vaegram)](https://docs.rs/vaegram)
[![Downloads](https://img.shields.io/crates/d/vaegram)](https://crates.io/crates/vaegram)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org)
[![License](https://img.shields.io/github/license/rjriajul/vaegram)](https://github.com/rjriajul/vaegram/blob/main/LICENSE)

</div>

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

vaegram talks to Telegram's main API over [MTProto](https://core.telegram.org/mtproto), the protocol the official
apps use. A bot can use it in place of the Bot API, and a user account can use it as a custom client. The transport,
encryption and session are written from scratch in Rust, and the types for every function of API layer 233 are
generated from Telegram's schema.

## Features

- Bot login with a token, and user login with a phone code, a 2FA password or a QR code.
- `on_*` handlers and hooks, with filters you combine with `&`, `|` and `!`.
- Conversations with `ask`, `listen`, `wait_for_click` and next-step handlers.
- Friendly methods for chats, messages, media, stories, payments, gifts and business accounts, and `invoke` for
  any raw function.
- HTML widgets over MTProto: bots send widgets and answer queries; user accounts read widgets, send queries and
  poll shared data. Shared-widget copies retain their id. Rendering HTML requires a host application.
- A session file that keeps the login, the chats the client has seen and the update state, so a restart picks up
  the updates it missed. SQLite, redb, PostgreSQL, Redis, MongoDB, S3 and Vault storage come behind cargo features.
- Transfers that survive a dead connection, a server error or a FLOOD_WAIT. Uploads, and downloads of a file whose
  size is known, go over several connections; a bot's transfers of 8 MiB or more are paced at what Telegram allows
  a bot (about 20 MB/s each way, measured with one bot).
- Rate limiting, FLOOD_WAIT retries, and reconnects that fill update gaps.
- An observer for every request, events for failures, and recorded traffic you can replay in tests.
- Crypto from [WarpCrypto](https://github.com/rjriajul/WarpCrypto).

## Installing

```toml
[dependencies]
vaegram = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

vaegram needs Rust 1.85 or newer. The `vae` crate is the same library under a shorter name: `vae = "0.3"`, then
`use vae::Client;`.

Get an `api_id` and `api_hash` from [my.telegram.org](https://my.telegram.org). A bot also needs a token from
[@BotFather](https://t.me/BotFather).

## Logging in as a user

Leave out `.bot_token(..)` and `start()` asks for the phone number, the login code and the 2FA password:

```rust
let app = Client::builder("my_account", api_id, api_hash).start().await?;
let me = app.get_me().await?;
println!("Logged in as {}", me.first_name);
app.send_message("me", "Hello from vaegram").await?;
```

## Examples

Buttons and callback queries ([examples/demo_bot.rs](examples/demo_bot.rs)):

```rust
app.on_message(filters::command("menu"), |msg| async move {
    let keyboard = InlineKeyboardMarkup::new([vec![
        InlineKeyboardButton::callback("Yes", "yes"),
        InlineKeyboardButton::callback("No", "no"),
    ]]);
    msg.answer("Pick one").reply_markup(keyboard).await?;
    Ok(())
});

app.on_callback_query(filters::all(), |query| async move {
    query.answer().text(format!("You pressed {}", query.data)).await?;
    Ok(())
});
```

A question and its answer:

```rust
app.on_message(filters::command("name"), |msg| async move {
    let answer = msg.client().ask(msg.chat.id, "What's your name?").filter(filters::text()).await?;
    answer.reply(format!("Hi {}", answer.text())).await?;
    Ok(())
});
```

Raw API calls and Telegram errors:

```rust
use vaegram::raw::functions;

let config = app.invoke(&functions::help::GetConfig {}).await?;

match app.send_message(chat_id, "hi").await {
    Err(e) if e.is("CHAT_WRITE_FORBIDDEN") => println!("no rights to write there"),
    Err(e) if e.flood_wait().is_some() => println!("rate limited"),
    other => { other?; }
}
```

Chat ids follow the Bot API: users are positive, groups negative, and channels start with `-100`. A method that takes
a chat also accepts `"me"`, `"@username"`, a `t.me` link, or a `&Chat`, `&User` or `&Message`.

## Sessions

The first run logs in and writes `my_bot.session`, and later runs reuse it. One client at a time can open a session
file. `.in_memory()` keeps nothing on disk, `.storage_engine(..)` stores the session anywhere, and
`export_session_string()` puts the login in a single line for an environment variable.

> A session file or string is the login itself. Anyone who has it is logged in as that account, so never commit or
> share it.

## Testing offline

`.record("bot.rec")` writes everything a client sends and receives, and `session::Replay` serves that file back on
localhost. Your handlers then run against real Telegram traffic with no network or credentials.
[examples/replay_test.rs](examples/replay_test.rs) shows the whole loop.

## Development

```bash
cargo test --workspace                                         # offline tests
cargo test --features unstable-raw --test live -- --ignored    # live tests against Telegram
cargo bench --bench tl                                         # benchmarks
```

## License

[Apache-2.0](LICENSE). Use it, fork it, change it, ship it. Every copy and fork must keep the [NOTICE](NOTICE) file
crediting vaegram and Riajul, and may not use the vaegram or vae name to promote itself.
