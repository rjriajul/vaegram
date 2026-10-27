<div align="center">

# vaegram

[![Crates.io](https://img.shields.io/crates/v/vaegram)](https://crates.io/crates/vaegram)
[![Docs.rs](https://img.shields.io/docsrs/vaegram)](https://docs.rs/vaegram)
[![Downloads](https://img.shields.io/crates/d/vaegram)](https://crates.io/crates/vaegram)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org)
[![License](https://img.shields.io/github/license/rjriajul/vaegram)](https://github.com/rjriajul/vaegram/blob/main/LICENSE)

**Fast, modern and asynchronous Telegram MTProto API framework in Rust for users and bots**

</div>

```rust
use vaegram::{Client, filters};

#[tokio::main]
async fn main() -> vaegram::Result<()> {
    let app = Client::builder("my_bot", 12345, "0123456789abcdef0123456789abcdef")
        .bot_token("123456:ABC-DEF")
        .start()
        .await?;

    app.on_message(filters::command("start") & filters::private(), |msg| async move {
        msg.reply("Hello from vaegram!").await?;
        Ok(())
    });

    app.run().await
}
```

**vaegram** is a [MTProto API](https://core.telegram.org/mtproto) framework written from scratch in
Rust. It talks to the main Telegram API directly, as a user account (custom client) or as a bot
(an alternative to the Bot API).

> **Status: early.** Bot login, handlers and filters, sending messages, raw API calls and sessions
> work and are tested against Telegram's servers. User login is next (see [Roadmap](#roadmap)).

### Key Features

- **Pure Rust** — Its own MTProto 2.0 transport, auth key handshake and encrypted session, with all of Telegram's security checks.
- **Up-to-Date** — All 868 functions of the latest layer (232), generated from the TL schema at build time.
- **Async Natively** — Built on [tokio](https://tokio.rs), `async`/`await` throughout.
- **Typed** — Every Telegram type and method is a Rust type, checked by the compiler.
- **Fast** — Crypto by [WarpCrypto](https://github.com/rjriajul/WarpCrypto), a high-performance cryptography library written in Rust.
- **Powerful** — Full access to Telegram's API through `app.invoke(...)`.

### Installing

```toml
[dependencies]
vaegram = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Requires Rust 1.85+. Prefer a shorter name? `vae = "0.1"` is the same crate: `use vae::Client;`.

You need an `api_id` and `api_hash` from [my.telegram.org](https://my.telegram.org), and for a bot,
a token from [@BotFather](https://t.me/BotFather).

### Handlers and Filters

`app.on_message(filter, callback)` runs `callback` for each new message that passes `filter`.
The first matching handler wins, so put specific ones first. `app.run()` keeps the bot running.

```rust
use vaegram::filters::{self, command, private, text};

app.on_message(command("start"), |msg| async move {
    msg.reply("Hi!").await?;
    Ok(())
});

app.on_message(text() & private() & !filters::bot(), |msg| async move {
    println!("{} said {}", msg.chat.id, msg.text);
    msg.reply(&msg.text).await?;
    Ok(())
});
```

Filters combine with `&`, `|` and `!`. Built in: `all`, `text`, `private`, `group`, `channel`,
`incoming`, `outgoing`, `bot`, `user(id)`, `chat(id)`, `command(name)`, `commands([...])`.
Make your own with `filters::create(|msg| msg.text.len() > 100)`.

A `Message` has `id`, `chat`, `from_user`, `sender_chat`, `date`, `text`, `outgoing`, `raw`
(the full Telegram object), `command()` (`"/echo hi"` gives `["echo", "hi"]`) and `reply(text)`.

Every update, including the ones without a friendly type yet, reaches `app.on_raw_update`:

```rust
app.on_raw_update(|app, update| async move {
    println!("{update:?}");
    Ok(())
});
```

Updates missed while connected (gaps in Telegram's sequence) are fetched again automatically.

### Sessions

The first run logs in and writes `my_bot.session`. Later runs reuse it, so you log in once.

You can also keep the whole session in one line, in an env var or a database:

```rust
let s = app.export_session_string(); // "VAE_..."

let app = Client::builder("my_bot", api_id, api_hash)
    .session_string(s)
    .start()
    .await?;
```

> A session file or string **is** the login: anyone who has it is logged in as that account.
> Never commit or share it. `*.session` is already in `.gitignore`.

### Raw API and Errors

Anything without a friendly method yet can be called directly:

```rust
use vaegram::{Error, raw::functions};

let config = app.invoke(&functions::help::GetConfig {}).await?;

match app.send_message(chat_id, "hi").await {
    Err(Error::Rpc(e)) if e.name == "FLOOD_WAIT" => println!("wait {} seconds", e.value.unwrap_or(0)),
    other => { other?; }
}
```

Chat ids use the Bot API style: users are positive, groups are negative, and channels and
supergroups start with `-100`.

### Roadmap

- [x] TL compiler, transport, auth key handshake, encrypted session
- [x] Bot login, data center migration, `send_message`, `get_me`, raw `invoke`
- [x] Session files and session strings
- [x] Update handlers and filters: `filters::command("start") & filters::private()`
- [x] Friendly types: `msg.text`, `msg.reply()`, `msg.from_user`
- [ ] More friendly types and handlers: edited messages, callback queries, inline queries
- [ ] User login (phone code, 2FA), auto-reconnect, file upload and download

### Development

```bash
# Clone the repo
git clone https://github.com/rjriajul/vaegram.git
cd vaegram

# Run the offline tests
cargo test --workspace

# Run the live tests against Telegram's servers
cargo test --test live -- --ignored
```

### Resources

- [Source code](https://github.com/rjriajul/vaegram)
- [Documentation](https://docs.rs/vaegram)
- [Issue tracker](https://github.com/rjriajul/vaegram/issues)

### License

[Apache-2.0](LICENSE). Use it, fork it, change it, ship it. Every copy and fork must keep the
[NOTICE](NOTICE) file crediting vaegram and Riajul, and may not use the vaegram or vae name to
promote itself.
