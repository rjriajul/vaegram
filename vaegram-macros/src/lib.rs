//! Derive macros for vaegram. Use them through vaegram with its `macros` feature, as
//! `vaegram::commands::BotCommands` or from `vaegram::prelude`, not from this crate.

mod attrs;
mod expand;
mod rename;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Writes `vaegram::commands::BotCommands` for an enum, one variant per command: `parse` reads message text into
/// a variant, `filter` matches the commands before an update is parsed, `bot_commands` is the menu for
/// `Client::set_bot_commands` and `descriptions` is the `/help` text.
///
/// ```ignore
/// use vaegram::prelude::*;
///
/// #[derive(BotCommands, Debug, PartialEq)]
/// #[command(description = "Commands:")]
/// enum Cmd {
///     /// Start the bot
///     Start(Option<String>),
///     /// Ban a user, with an optional reason
///     #[command(alias = "b")]
///     Ban { user_id: i64, #[command(rest)] reason: Option<String> },
///     /// Show this help
///     #[command(allow_extra)]
///     Help,
///     #[command(hide)]
///     Debug,
/// }
///
/// assert_eq!(Cmd::parse("/ban@my_bot 42 spam bot", Some("my_bot"))?, Cmd::Ban {
///     user_id: 42,
///     reason: Some("spam bot".into()),
/// });
/// ```
///
/// # Names
///
/// A variant's command is its name in `snake_case`: `SetAge` is `/set_age`. Words split at `_` and before an
/// uppercase letter that ends a lowercase word or a digit, or that starts a new word after an acronym, so
/// `HTTPStatus` is `/http_status` and `Top10` is `/top10`. Commands match in any letter case, as
/// `filters::command` does, unless the enum is `case_sensitive`. `/cmd@bot` is taken only when `bot` is the
/// `bot_username` given to `parse`, which `Message::parse_command` fills in.
///
/// # Arguments
///
/// A command with one field reads all the text after it, spaces included, with the field's `FromStr`: `/echo hello
/// world` is `Echo("hello world")`. An `Option` field is `None` when there's no text, and any other field needs
/// some. With several fields, the text splits on whitespace, one word per field in order, each parsed with its
/// `FromStr`; `Option` fields may be left out, but only at the end, and a last field marked `#[command(rest)]`
/// takes all the remaining text instead of one word. A command without fields refuses any text after it, unless
/// it's `allow_extra`. Trailing whitespace is never part of an argument. Too few or too many words are
/// `TooFewArguments` or `TooManyArguments`, and a word that doesn't parse is `InvalidArgument`, which names the
/// field.
///
/// # Enum settings: `#[command(...)]` on the enum
///
/// - `rename_rule = "snake_case"`: how variant names become commands: `"snake_case"` (the default),
///   `"lowercase"`, `"UPPERCASE"`, `"PascalCase"`, `"camelCase"`, `"SCREAMING_SNAKE_CASE"` or `"identity"`.
/// - `prefix = "!"` or `prefixes = ["/", "!"]`: what starts a command; `/` by default. The first one shows in
///   the help text.
/// - `case_sensitive`: `/Start` is no longer `/start`.
/// - `description = "..."`: the help text's header. The enum's doc comment is left to rustdoc.
/// - `hide_menu`: keeps every command out of the menu, for a bot that never sets one.
/// - `crate = "vae"`: the crate generated code names, for an app that depends on `vae` instead of `vaegram`.
///
/// # Command settings: `#[command(...)]` on a variant
///
/// - `///` doc comment, or `description = "..."`: what the command does, for the menu and the help text. Only
///   the doc comment's first paragraph is used.
/// - `rename = "name"`: the command's name, as written.
/// - `alias = "b"` or `aliases = ["b", "kick"]`: other names it answers to. They are in the help text, not the
///   menu.
/// - `hide`: left out of the menu and the help text, still parsed.
/// - `hide_menu`: left out of the menu only.
/// - `hide_aliases`: aliases left out of the help text.
/// - `ephemeral`: its menu entry makes the message seen only by its sender and the bot.
/// - `allow_extra`: a command without fields ignores the text after it.
/// - `parse_with = path::to::parser`: a `fn(&str) -> Result<T, E>` that gets all the text after the command,
///   without trailing whitespace, where `T` is the field's type, or a tuple of the fields' types for several,
///   and `E` converts into `Box<dyn Error + Send + Sync>`, as `&str`, `String` and every error type do. A
///   `CommandParseError` it returns comes out as itself; anything else is `CommandParseError::Custom`.
///
/// # Checked when it builds
///
/// Telegram refuses a whole menu with one bad entry, so a command in the menu (not `hide` or `hide_menu`) must be
/// 1 to 32 lowercase English letters, digits or underscores, with a description of 3 to 256 characters, and the
/// menu holds at most 100 commands. Every name and alias must be 1 to 32 letters, digits or underscores and
/// name one command only. Each of these, and every misplaced setting, is a build error pointing at its cause.
///
/// A generic enum works, as long as no type parameter is named `__VaegramText`: the generated `filter` uses that
/// name for its own.
#[proc_macro_derive(BotCommands, attributes(command))]
pub fn derive_bot_commands(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand::bot_commands(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}
