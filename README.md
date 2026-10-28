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
        msg.answer("Hello from vaegram!").await?;
        Ok(())
    });

    app.run().await
}
```

**vaegram** is a [MTProto API](https://core.telegram.org/mtproto) framework written from scratch in
Rust. It talks to the main Telegram API directly, as a user account (custom client) or as a bot
(an alternative to the Bot API).

> **Status: early.** User and bot login (with 2FA), handlers and filters, sending messages, raw API
> calls and sessions work and are tested against Telegram's servers (see [Roadmap](#roadmap)).

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

### User Accounts

Leave out `.bot_token(...)` and `start()` asks for the phone number, the code and the 2FA password.

```rust
let app = Client::builder("my_account", api_id, api_hash).start().await?;
```

More in [examples](examples).

### Sessions

The first run logs in and writes `my_bot.session`. Later runs reuse it, so you log in once.
Next to it, `my_bot.state` keeps known chats and the update state, so after a restart you can
still message chats by id and `run()` delivers the updates you missed while offline.

You can also keep the whole session in one line, in an env var or a database (the line holds
only the login, so known chats and missed updates aren't kept across restarts):

```rust
let s = app.export_session_string(); // "VAE_..."

let app = Client::builder("my_bot", api_id, api_hash)
    .session_string(s)
    .start()
    .await?;
```

> A session file or string **is** the login: anyone who has it is logged in as that account.
> Never commit or share it. `*.session` and `*.state` are already in `.gitignore`.

`.test_mode()` logs in on Telegram's test servers and `.ipv6()` connects over IPv6. Session strings remember
the test mode and the data center address.

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
supergroups start with `-100`. Anywhere a method takes a chat or user you can also pass `"me"`, `"@username"`,
a `t.me` link (`t.me/name`, `t.me/c/123/4`) or a `&Chat`, `&User` or `&Message`. Invite links (`t.me/+…`) join with
`join_chat`; other methods accept them once you're a member.

### Roadmap

- [x] TL compiler, transport, auth key handshake, encrypted session
- [x] Bot login, data center migration, `send_message`, `get_me`, raw `invoke`
- [x] Session files and session strings
- [x] Update handlers and filters: `filters::command("start") & filters::private()`
- [x] Friendly types: `msg.text`, `msg.reply()`, `msg.from_user`
- [ ] Message media and metadata: `msg.caption`, `msg.entities`, `msg.photo()`, `msg.poll()`, replies and forwards (not yet tested live)
- [x] HTML and Markdown formatting: `send_message(chat, "<b>hi</b>").html()`
- [x] Buttons, edited messages, callback queries, inline queries, `edit_text`, `delete`
- [x] User login (phone code, 2FA password)
- [x] Auto-reconnect, short `FLOOD_WAIT`s slept off
- [ ] Abridged and padded transports, obfuscation, SOCKS5 and MTProxy: `.transport(Transport::Abridged)`, `.proxy(..)` (not yet tested live)
- [ ] Chats and members: `get_chat`, `get_chat_members`, `ban_chat_member`, `promote_chat_member`, `join_chat` (not yet tested live)
- [ ] Message history and search, get, forward, copy, pin, read, chat actions; received keyboards via `msg.reply_markup()` (not yet tested live)
- [ ] Locations, venues, contacts, dice and polls: `send_poll(chat, "Lunch?", ["Pizza", "Sushi"]).quiz([0])` (not yet tested live)
- [ ] Parallel file upload and download: `send_document`, `send_photo`, `msg.download()` (not yet tested live)
- [ ] Shared send options: replies with quotes, topics, silent, protected, scheduled and business sends; a default
  parse mode via `Client::builder(..).parse_mode(enums::ParseMode::Html)` (not yet tested live)
- [ ] Chat join requests and join request queries, managed bot access settings (not yet tested live)
- [ ] Invite links and join requests: `create_chat_invite_link(chat).creates_join_request()`, `request.approve()`, Star subscription links (not yet tested live)
- [ ] Managed bots: `on_managed_bot`, token get/replace, `KeyboardButton::request_managed_bot`,
  `ServiceAction::ManagedBotCreated` (not yet tested live)
- [ ] Client events: reconnects, dropped packets, skipped or unknown updates and failed handlers via `app.on_event(|e| ..)` (not yet tested live)
- [ ] Bot commands, reaction removal, personal channel messages, Stars subscription updates (not yet tested live)
- [ ] Communities: `msg.service` (`ServiceAction::CommunityChatAdded`, ..), `get_chat(..).community` (not yet tested live)
- [ ] Message drafts and streaming: `stream_message(chat).can_stop()`, `on_message_generation_stopped` (not yet tested live)
- [ ] Bot profile: name, descriptions, menu button, default admin rights, `log_out` (not yet tested live)
- [ ] Every inline result kind, chosen inline results, `answer_web_app_query`, prepared inline messages (not yet tested
  live)
- [ ] Suggested posts and channel direct messages: `approve_suggested_post(chat, id).send_date(..)`,
  `.suggested_post_parameters(..)`, `.direct_messages_topic_id(user)`, `msg.direct_messages_topic` (not yet tested live)
- [ ] Ephemeral messages: send, edit, delete, handlers and callback queries (not yet tested live)
- [ ] Reactions: `send_reaction`, `msg.react(..)`, `msg.reactions`, `on_message_reaction`, `on_message_reaction_count` (not yet tested live)
- [ ] Forum topics, also in private chats with bots: `create_forum_topic(chat, "Ideas")`, `msg.service`
  (`ServiceAction::ForumTopicCreated`, ..), `msg.message_thread_id` (not yet tested live)
- [ ] Business accounts: `on_business_message`, `on_business_connection`, `on_deleted_business_messages`, replies on
  behalf of the account, `read_business_message`, name, bio, username, gift settings and Stars (not yet tested live)
- [ ] Rich messages: `send_rich_message(chat, InputRichMessage::html("<b>hi</b>"))`, rich drafts and edits,
  `msg.rich_message` (not yet tested live)
- [ ] Gifts and verification: `send_gift(chat, id)`, `get_chat_gifts(chat)`, `upgrade_gift(id)`, `transfer_gift(id, chat)`,
  `msg.service` (`ServiceAction::Gift`, ..), `verify_chat(chat)` (not yet tested live)
- [ ] Checklists: `send_checklist(chat, InputChecklist::new("Trip").task(1, "Tickets"))`, `edit_message_checklist`,
  `msg.checklist()`, `msg.service` (`ServiceAction::ChecklistTasksDone`, ..) (not yet tested live)
- [ ] Games: `send_game(chat, "race")`, `set_game_score`, `get_game_high_scores`, `msg.game()`, inline callback
  queries and their game variants on the message's own data center (not yet tested live)
- [ ] Boosts, profile photos and stories: `get_user_chat_boosts(chat, user)`, `on_chat_boost`, `on_removed_chat_boost`,
  `ServiceAction::BoostAdded`, `get_chat_photos(user)`, `set_user_emoji_status(user)`, business `post_story`,
  `edit_story`, `repost_story`, `delete_story` (not yet tested live)
- [ ] Chat member, poll, poll answer, deleted message and user status handlers, business callback queries
  (not yet tested live)
- [ ] Chat and profile photos: `set_chat_photo(chat, "a.jpg")`, `set_profile_photo("a.jpg").video()`,
  `delete_chat_photo`, `remove_profile_photo`, business account photos (not yet tested live)
- [ ] Sticker sets: `create_new_sticker_set`, `add_sticker_to_set`, `upload_sticker_file`, `get_sticker_set`,
  `get_custom_emoji_stickers`, `set_chat_sticker_set` (not yet tested live)
- [ ] Shortcuts: `msg.reply()` always quotes and `msg.answer()` never does, both staying in the topic; `msg.forward`,
  `copy`, `pin`, `unpin`, `edit_caption`, `edit_reply_markup`; `query.edit_message_reply_markup`,
  `query.delete_message`; `request.approve()` answers guard bot queries (not yet tested live)

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
