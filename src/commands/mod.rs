//! Bot commands as an enum: the [`BotCommands`](trait@BotCommands) trait, its parse error and the `/help` text.
//!
//! A type implementing [`BotCommands`](trait@BotCommands) parses message text into one of its commands, gives the
//! dispatcher a [`Filter`] that matches its commands before any update is parsed, lists its commands for the bot's
//! menu and renders a help text. The helpers below ([`parse_header`], [`name_matches`], [`split_args`],
//! [`parse_arg`], [`parse_rest`] and their variants) are what an implementation calls, so hand-written impls and
//! generated ones read commands the same way as [`filters::command`](crate::filters::command) and
//! [`Message::command`](crate::types::Message::command).
//!
//! ```
//! use vaegram::commands::{
//!     BotCommands, CommandDescription, CommandDescriptions, CommandParseError, name_matches, parse_arg,
//!     parse_header, parse_opt_rest, split_args,
//! };
//! use vaegram::filters::HasText;
//! use vaegram::prelude::*;
//! use vaegram::types::BotCommand;
//!
//! enum Cmd {
//!     Start(Option<String>),
//!     Ban(i64),
//! }
//!
//! static HELP: [CommandDescription<'static>; 2] =
//!     [CommandDescription::new("/", "start", "Start the bot"), CommandDescription::new("/", "ban", "Ban a user")];
//!
//! impl BotCommands for Cmd {
//!     fn parse(text: &str, bot_username: Option<&str>) -> Result<Self, CommandParseError> {
//!         let command = parse_header(text, &["/"], bot_username)?;
//!         if name_matches(command.name, "start") {
//!             Ok(Cmd::Start(parse_opt_rest(command.rest(), "payload")?))
//!         } else if name_matches(command.name, "ban") {
//!             let args = split_args(command.rest(), 1, 1)?;
//!             Ok(Cmd::Ban(parse_arg(args[0], 0, "user_id")?))
//!         } else {
//!             Err(CommandParseError::UnknownCommand(command.name.to_owned()))
//!         }
//!     }
//!
//!     fn filter<T: HasText>() -> Filter<T> {
//!         filters::command(["start", "ban"])
//!     }
//!
//!     fn bot_commands() -> Vec<BotCommand> {
//!         HELP.iter().map(|d| BotCommand::new(d.command, d.description)).collect()
//!     }
//!
//!     fn descriptions() -> CommandDescriptions<'static> {
//!         CommandDescriptions::new(&HELP)
//!     }
//! }
//!
//! assert!(matches!(Cmd::parse("/start", None), Ok(Cmd::Start(None))));
//! assert!(matches!(Cmd::parse("/ban@MyBot 42", Some("mybot")), Ok(Cmd::Ban(42))));
//! assert!(matches!(Cmd::parse("/ban@other 42", Some("mybot")), Err(CommandParseError::WrongBot(_))));
//! assert_eq!(Cmd::descriptions().to_string(), "/start - Start the bot\n/ban - Ban a user");
//! ```
//!
//! With the `macros` feature, `#[derive(BotCommands)]` writes the impl from the enum: names from the variants,
//! descriptions from their doc comments and arguments from their fields, with the menu's rules checked when it
//! builds. The derive's docs list every setting.
//!
//! ```
//! # #[cfg(feature = "macros")] {
//! use vaegram::prelude::*;
//!
//! #[derive(BotCommands, Debug, PartialEq)]
//! enum Cmd {
//!     /// Start the bot
//!     Start(Option<String>),
//!     /// Ban a user, with an optional reason
//!     #[command(alias = "b")]
//!     Ban { user_id: i64, #[command(rest)] reason: Option<String> },
//! }
//!
//! let ban = Cmd::parse("/b@MyBot 42 spam bot", Some("mybot")).unwrap();
//! assert_eq!(ban, Cmd::Ban { user_id: 42, reason: Some("spam bot".into()) });
//! let help = Cmd::descriptions().to_string();
//! assert_eq!(help, "/start - Start the bot\n/ban, /b - Ban a user, with an optional reason");
//! # }
//! ```

use std::error::Error as StdError;
use std::fmt;
use std::str::FromStr;

use crate::filters::{Filter, HasText};
use crate::types::{BotCommand, BotCommandScope, Command};

// The derive shares the trait's name, so one `use` brings in both.
#[cfg(feature = "macros")]
pub use vaegram_macros::BotCommands;

/// A set of bot commands, usually an enum with one variant per command.
///
/// Implementations compare command names with [`name_matches`], or with `==` only when the set is case sensitive and
/// its filter is too. The command filters match names lowercased in any script, so this keeps the filter taking
/// every text `parse` takes.
pub trait BotCommands: Sized {
    /// Parses `text` into a command. With `bot_username` set, `/cmd@that_bot` parses and `/cmd@other_bot` is
    /// [`CommandParseError::WrongBot`]; with `None`, as before the client knows its own username, a bare `/cmd`
    /// still parses and any `@mention` is `WrongBot`.
    fn parse(text: &str, bot_username: Option<&str>) -> Result<Self, CommandParseError>;

    /// A filter matching this set's command names and aliases, hidden ones included, checked before the update is
    /// parsed. It must accept every text [`parse`](BotCommands::parse) can read.
    fn filter<T: HasText>() -> Filter<T>;

    /// The commands to list in the bot's menu with `Client::set_bot_commands`; hidden commands are left out.
    /// Telegram refuses the whole menu if any description is shorter than 3 or longer than 256 characters.
    fn bot_commands() -> Vec<BotCommand>;

    /// The menu for `scope`. All of [`bot_commands`](BotCommands::bot_commands) unless the set limits commands to
    /// scopes.
    fn bot_commands_for(_scope: &BotCommandScope) -> Vec<BotCommand> {
        Self::bot_commands()
    }

    /// The help text, shown with `to_string()`.
    fn descriptions() -> CommandDescriptions<'static>;
}

/// Why text didn't parse as a command of a [`BotCommands`](trait@BotCommands) set.
#[derive(Debug)]
#[non_exhaustive]
pub enum CommandParseError {
    /// The text doesn't start with a command prefix followed by a valid name.
    NotACommand,
    /// The command names another bot, the one held here.
    WrongBot(String),
    /// No command of the set has this name.
    UnknownCommand(String),
    /// Fewer arguments than the command needs.
    TooFewArguments {
        /// Least number of arguments the command takes.
        expected: usize,
        /// Number of arguments given.
        found: usize,
    },
    /// More arguments than the command takes.
    TooManyArguments {
        /// Most arguments the command takes.
        expected: usize,
        /// Number of arguments given.
        found: usize,
    },
    /// An argument didn't parse into its field's type.
    InvalidArgument {
        /// Zero-based position of the argument.
        index: usize,
        /// Name of the field it was parsed for, shown in the error text.
        field: &'static str,
        /// The type's own parse error.
        source: Box<dyn StdError + Send + Sync>,
    },
    /// A custom argument parser failed.
    Custom(Box<dyn StdError + Send + Sync>),
}

impl CommandParseError {
    /// The error of a custom argument parser. A [`CommandParseError`] comes back as itself; anything else, such as
    /// a `String`, a `&str` or another error type, becomes [`CommandParseError::Custom`].
    pub fn custom<E: Into<Box<dyn StdError + Send + Sync>>>(error: E) -> Self {
        let error: Box<dyn StdError + Send + Sync> = error.into();
        match error.downcast::<CommandParseError>() {
            Ok(error) => *error,
            Err(error) => CommandParseError::Custom(error),
        }
    }
}

fn arguments(n: usize) -> &'static str {
    if n == 1 { "argument" } else { "arguments" }
}

impl fmt::Display for CommandParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandParseError::NotACommand => f.write_str("not a command"),
            CommandParseError::WrongBot(bot) => write!(f, "the command is for @{bot}"),
            CommandParseError::UnknownCommand(name) => write!(f, "unknown command {name}"),
            CommandParseError::TooFewArguments { expected, found } => {
                write!(f, "expected at least {expected} {}, got {found}", arguments(*expected))
            }
            CommandParseError::TooManyArguments { expected, found } => {
                write!(f, "expected at most {expected} {}, got {found}", arguments(*expected))
            }
            CommandParseError::InvalidArgument { field, source, .. } => write!(f, "invalid {field}: {source}"),
            CommandParseError::Custom(e) => write!(f, "{e}"),
        }
    }
}

impl StdError for CommandParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            CommandParseError::InvalidArgument { source, .. } => Some(&**source),
            CommandParseError::Custom(e) => Some(&**e),
            _ => None,
        }
    }
}

/// Lets a handler returning `vaegram::Result` use `?` on a parse; it becomes [`Error::Other`](crate::Error::Other).
impl From<CommandParseError> for crate::Error {
    fn from(e: CommandParseError) -> Self {
        crate::Error::other(e)
    }
}

/// Reads the command at the start of `text`, as
/// [`Message::command_with_prefixes`](crate::types::Message::command_with_prefixes) does: the longest of `prefixes`
/// starts it and empty prefixes never match. The name and arguments split on whitespace only. A mention of
/// another bot than `bot_username`, or any mention when it's `None`, is [`CommandParseError::WrongBot`].
pub fn parse_header<'a, S: AsRef<str>>(
    text: &'a str,
    prefixes: &[S],
    bot_username: Option<&str>,
) -> Result<Command<'a>, CommandParseError> {
    let command = parse_header_unchecked(text, prefixes)?;
    match command.bot {
        Some(bot) if !command.is_for(bot_username) => Err(CommandParseError::WrongBot(bot.to_owned())),
        _ => Ok(command),
    }
}

/// Like [`parse_header`], but any `@mention` is accepted, for a set that answers commands meant for other bots.
/// The command filters drop a mention of another bot before the handler runs, so such a set's
/// [`filter`](BotCommands::filter) can't be one of them: use one that doesn't read mentions, such as
/// [`filters::text_starts_with`](crate::filters::text_starts_with).
pub fn parse_header_unchecked<'a, S: AsRef<str>>(
    text: &'a str,
    prefixes: &[S],
) -> Result<Command<'a>, CommandParseError> {
    Command::parse_prefixed(text, prefixes).ok_or(CommandParseError::NotACommand)
}

/// Whether the typed command name `typed` is `name` in any letter case, compared the way the command filters
/// compare names: lowercased char by char, in any script.
pub fn name_matches(typed: &str, name: &str) -> bool {
    typed.chars().flat_map(char::to_lowercase).eq(name.chars().flat_map(char::to_lowercase))
}

/// Splits `rest` on whitespace into `min` to `max` arguments.
pub fn split_args(rest: &str, min: usize, max: usize) -> Result<Vec<&str>, CommandParseError> {
    let args: Vec<&str> = rest.split_whitespace().collect();
    if args.len() < min {
        return Err(CommandParseError::TooFewArguments { expected: min, found: args.len() });
    }
    if args.len() > max {
        return Err(CommandParseError::TooManyArguments { expected: max, found: args.len() });
    }
    Ok(args)
}

/// Splits `rest` into its first `n` arguments and the text after them, trimmed, for a command whose last field
/// takes the rest of the text: `42 spam bot` with `n = 1` is `["42"]` and `spam bot`. Fewer than `n` arguments,
/// or no text left when `tail_required`, is [`CommandParseError::TooFewArguments`], counting a required tail as
/// one more argument.
pub fn split_args_rest(rest: &str, n: usize, tail_required: bool) -> Result<(Vec<&str>, &str), CommandParseError> {
    let expected = n + usize::from(tail_required);
    let mut args = Vec::with_capacity(n);
    let mut tail = rest.trim();
    while args.len() < n {
        if tail.is_empty() {
            return Err(CommandParseError::TooFewArguments { expected, found: args.len() });
        }
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        args.push(&tail[..end]);
        tail = tail[end..].trim_start();
    }
    if tail_required && tail.is_empty() {
        return Err(CommandParseError::TooFewArguments { expected, found: n });
    }
    Ok((args, tail))
}

/// Parses the argument at `index` with its [`FromStr`]. `field` names the field it fills, such as `user_id`, and
/// shows in the error text.
pub fn parse_arg<T>(arg: &str, index: usize, field: &'static str) -> Result<T, CommandParseError>
where
    T: FromStr,
    T::Err: StdError + Send + Sync + 'static,
{
    arg.parse().map_err(|e| CommandParseError::InvalidArgument { index, field, source: Box::new(e) })
}

/// Like [`parse_arg`], for an optional field: a missing argument is `None`.
pub fn parse_opt_arg<T>(arg: Option<&str>, index: usize, field: &'static str) -> Result<Option<T>, CommandParseError>
where
    T: FromStr,
    T::Err: StdError + Send + Sync + 'static,
{
    arg.map(|arg| parse_arg(arg, index, field)).transpose()
}

/// Parses everything after the command, `rest`, without trailing whitespace, as the command's one field. No text
/// at all is [`CommandParseError::TooFewArguments`], so a required `String` field is never silently empty.
pub fn parse_rest<T>(rest: &str, field: &'static str) -> Result<T, CommandParseError>
where
    T: FromStr,
    T::Err: StdError + Send + Sync + 'static,
{
    let rest = rest.trim();
    if rest.is_empty() {
        return Err(CommandParseError::TooFewArguments { expected: 1, found: 0 });
    }
    parse_arg(rest, 0, field)
}

/// Like [`parse_rest`], for an optional field: nothing after the command is `None`.
pub fn parse_opt_rest<T>(rest: &str, field: &'static str) -> Result<Option<T>, CommandParseError>
where
    T: FromStr,
    T::Err: StdError + Send + Sync + 'static,
{
    let rest = rest.trim();
    parse_opt_arg(Some(rest).filter(|rest| !rest.is_empty()), 0, field)
}

/// One command's line in the help text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandDescription<'a> {
    /// Prefix shown before the name, such as `/`.
    pub prefix: &'a str,
    /// Command name.
    pub command: &'a str,
    /// Other names the command answers to.
    pub aliases: &'a [&'a str],
    /// What the command does; empty for none. A description that also goes to the bot's menu must be 3 to 256
    /// characters long, or Telegram refuses the menu.
    pub description: &'a str,
}

impl<'a> CommandDescription<'a> {
    /// A command without aliases.
    pub const fn new(prefix: &'a str, command: &'a str, description: &'a str) -> Self {
        CommandDescription { prefix, command, aliases: &[], description }
    }

    /// Sets the aliases shown after the name.
    #[must_use]
    pub const fn aliases(self, aliases: &'a [&'a str]) -> Self {
        CommandDescription { aliases, ..self }
    }
}

impl fmt::Display for CommandDescription<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.prefix, self.command)?;
        for alias in self.aliases {
            write!(f, ", {}{alias}", self.prefix)?;
        }
        if !self.description.is_empty() {
            write!(f, " - {}", self.description)?;
        }
        Ok(())
    }
}

/// The help text of a [`BotCommands`](trait@BotCommands) set: an optional header, then one line per shown command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandDescriptions<'a> {
    /// Text before the command list.
    pub header: Option<&'a str>,
    /// The shown commands, in order.
    pub commands: &'a [CommandDescription<'a>],
}

impl<'a> CommandDescriptions<'a> {
    /// A help text without a header.
    pub const fn new(commands: &'a [CommandDescription<'a>]) -> Self {
        CommandDescriptions { header: None, commands }
    }

    /// Sets the text shown before the command list.
    #[must_use]
    pub const fn header(self, header: &'a str) -> Self {
        CommandDescriptions { header: Some(header), ..self }
    }
}

impl fmt::Display for CommandDescriptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(header) = self.header {
            f.write_str(header)?;
            if !self.commands.is_empty() {
                f.write_str("\n\n")?;
            }
        }
        for (i, command) in self.commands.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{command}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::filters::{self, Facts};
    use crate::types::Message;

    /// What the derive would generate for:
    /// `enum Cmd { Start(Option<String>), Ban { user_id: i64, minutes: Option<u32> }, Help, #[hide] Debug(String) }`
    #[derive(Debug, PartialEq)]
    enum Cmd {
        Start(Option<String>),
        Ban { user_id: i64, minutes: Option<u32> },
        Help,
        Debug(String),
    }

    static SHOWN: [CommandDescription<'static>; 3] = [
        CommandDescription::new("/", "start", "Start the bot"),
        CommandDescription::new("/", "ban", "Ban a user").aliases(&["b"]),
        CommandDescription::new("/", "help", "Show this help"),
    ];

    impl BotCommands for Cmd {
        fn parse(text: &str, bot_username: Option<&str>) -> Result<Self, CommandParseError> {
            let command = parse_header(text, &["/"], bot_username)?;
            let name = command.name;
            if name.eq_ignore_ascii_case("start") {
                Ok(Cmd::Start(parse_opt_rest(command.rest(), "payload")?))
            } else if name.eq_ignore_ascii_case("ban") || name.eq_ignore_ascii_case("b") {
                let args = split_args(command.rest(), 1, 2)?;
                Ok(Cmd::Ban {
                    user_id: parse_arg(args[0], 0, "user_id")?,
                    minutes: parse_opt_arg(args.get(1).copied(), 1, "minutes")?,
                })
            } else if name.eq_ignore_ascii_case("help") {
                split_args(command.rest(), 0, 0)?;
                Ok(Cmd::Help)
            } else if name.eq_ignore_ascii_case("debug") {
                Ok(Cmd::Debug(parse_rest(command.rest(), "dump")?))
            } else {
                Err(CommandParseError::UnknownCommand(name.to_owned()))
            }
        }

        fn filter<T: HasText>() -> Filter<T> {
            filters::command(["start", "ban", "b", "help", "debug"])
        }

        fn bot_commands() -> Vec<BotCommand> {
            SHOWN.iter().map(|d| BotCommand::new(d.command, d.description)).collect()
        }

        fn descriptions() -> CommandDescriptions<'static> {
            CommandDescriptions::new(&SHOWN).header("Commands:")
        }
    }

    fn passes(text: &str, me: Option<&str>) -> Option<bool> {
        Cmd::filter::<Message>().decide(&Facts { text, me, ..Facts::default() })
    }

    #[test]
    fn parses_without_and_with_args() {
        assert_eq!(Cmd::parse("/start", None).unwrap(), Cmd::Start(None));
        assert_eq!(Cmd::parse("/start  abc", None).unwrap(), Cmd::Start(Some("abc".into())));
        assert_eq!(Cmd::parse("/help", None).unwrap(), Cmd::Help);
        assert_eq!(Cmd::parse("/ban 42", None).unwrap(), Cmd::Ban { user_id: 42, minutes: None });
        assert_eq!(Cmd::parse("/B  42   5", None).unwrap(), Cmd::Ban { user_id: 42, minutes: Some(5) });
        assert_eq!(Cmd::parse("/debug a b", None).unwrap(), Cmd::Debug("a b".into()));
        // One field reads all the text, without trailing whitespace, as the derive reads it.
        assert_eq!(Cmd::parse("/start a b ", None).unwrap(), Cmd::Start(Some("a b".into())));
        assert_eq!(Cmd::parse("/debug a b \n", None).unwrap(), Cmd::Debug("a b".into()));
        assert!(matches!(
            Cmd::parse("/debug  ", None),
            Err(CommandParseError::TooFewArguments { expected: 1, found: 0 })
        ));
    }

    #[test]
    fn mention_must_name_this_bot() {
        assert_eq!(Cmd::parse("/help@MyBot", Some("mybot")).unwrap(), Cmd::Help);
        let wrong = Cmd::parse("/help@other", Some("mybot"));
        assert!(matches!(wrong, Err(CommandParseError::WrongBot(b)) if b == "other"));
        // Before the client knows its username a bare command parses and any mention is refused.
        assert!(matches!(Cmd::parse("/help@mybot", None), Err(CommandParseError::WrongBot(_))));
    }

    #[test]
    fn unchecked_header_takes_any_mention() {
        let command = parse_header_unchecked("/help@other now", &["/"]).unwrap();
        assert_eq!((command.name, command.bot, command.rest()), ("help", Some("other"), "now"));
        assert!(matches!(parse_header_unchecked("help", &["/"]), Err(CommandParseError::NotACommand)));
    }

    #[test]
    fn unknown_and_non_commands_fail() {
        assert!(matches!(Cmd::parse("/nope", None), Err(CommandParseError::UnknownCommand(n)) if n == "nope"));
        assert!(matches!(Cmd::parse("hello", None), Err(CommandParseError::NotACommand)));
        assert!(matches!(Cmd::parse("/", None), Err(CommandParseError::NotACommand)));
    }

    #[test]
    fn argument_count_and_type_errors() {
        use CommandParseError::{TooFewArguments, TooManyArguments};
        assert!(matches!(Cmd::parse("/ban", None), Err(TooFewArguments { expected: 1, found: 0 })));
        assert!(matches!(Cmd::parse("/ban 1 2 3", None), Err(TooManyArguments { expected: 2, found: 3 })));
        assert!(matches!(Cmd::parse("/help me", None), Err(TooManyArguments { expected: 0, found: 1 })));
        let err = Cmd::parse("/ban 42 soon", None).unwrap_err();
        assert!(matches!(err, CommandParseError::InvalidArgument { index: 1, field: "minutes", .. }));
        assert!(err.source().is_some());
        assert!(err.to_string().starts_with("invalid minutes: "));
        let err: crate::Error = err.into();
        assert!(err.downcast_ref::<CommandParseError>().is_some());
    }

    #[test]
    fn error_text_counts_arguments() {
        let text = |text| Cmd::parse(text, None).unwrap_err().to_string();
        assert_eq!(text("/ban"), "expected at least 1 argument, got 0");
        assert_eq!(text("/ban 1 2 3"), "expected at most 2 arguments, got 3");
        assert_eq!(text("/help me"), "expected at most 0 arguments, got 1");
        assert_eq!(text("/nope"), "unknown command nope");
        assert_eq!(text("hello"), "not a command");
    }

    #[test]
    fn help_text_lists_shown_commands() {
        assert_eq!(
            Cmd::descriptions().to_string(),
            "Commands:\n\n/start - Start the bot\n/ban, /b - Ban a user\n/help - Show this help"
        );
        assert_eq!(CommandDescriptions::new(&[]).header("Nothing").to_string(), "Nothing");
        assert_eq!(CommandDescription::new("/", "x", "").to_string(), "/x");
    }

    #[test]
    fn bot_commands_skip_hidden() {
        let names: Vec<String> = Cmd::bot_commands().into_iter().map(|c| c.command).collect();
        assert_eq!(names, ["start", "ban", "help"]);
        assert_eq!(Cmd::bot_commands_for(&BotCommandScope::Default), Cmd::bot_commands());
    }

    #[test]
    fn filter_takes_what_parse_takes() {
        assert_eq!(passes("/debug x", None), Some(true));
        assert_eq!(passes("/b 1", None), Some(true));
        assert_eq!(passes("/nope", None), Some(false));
        assert_eq!(passes("/help@other", Some("mybot")), Some(false));
        assert_eq!(passes("/help@MyBot", Some("mybot")), Some(true));
        for shown in &SHOWN {
            for name in std::iter::once(&shown.command).chain(shown.aliases) {
                assert_eq!(passes(&format!("/{name}"), None), Some(true), "{name}");
            }
        }
    }

    #[test]
    fn case_sensitive_prefixes_filter() {
        let filter = filters::command_with_prefixes_case_sensitive::<Message>(["/", "!x"], "start");
        let decide = |text| filter.decide(&Facts { text, ..Facts::default() });
        assert_eq!((decide("!xstart"), decide("/start"), decide("!xStart")), (Some(true), Some(true), Some(false)));
    }

    #[test]
    fn names_match_the_way_the_filter_matches() {
        assert!(name_matches("START", "start") && name_matches("Старт", "старт"));
        assert!(!name_matches("star", "start"));
        // The Kelvin sign lowercases to `k`, so the filter takes `/Kick` written with it; `parse` must take it too.
        assert!(name_matches("\u{212A}ick", "kick"));
        let filter = filters::command::<Message>("kick");
        assert_eq!(filter.decide(&Facts { text: "/\u{212A}ick", ..Facts::default() }), Some(true));
    }

    #[test]
    fn split_args_rest_keeps_the_tail_whole() {
        assert_eq!(split_args_rest("42  spam  bot ", 1, true).unwrap(), (vec!["42"], "spam  bot"));
        assert_eq!(split_args_rest("a b", 2, false).unwrap(), (vec!["a", "b"], ""));
        assert_eq!(split_args_rest("  hi there", 0, true).unwrap(), (vec![], "hi there"));
        assert_eq!(split_args_rest("", 0, false).unwrap(), (vec![], ""));
        let err = split_args_rest("42", 1, true).unwrap_err();
        assert!(matches!(err, CommandParseError::TooFewArguments { expected: 2, found: 1 }), "{err}");
        let err = split_args_rest(" 1 ", 3, false).unwrap_err();
        assert!(matches!(err, CommandParseError::TooFewArguments { expected: 3, found: 1 }), "{err}");
        let err = split_args_rest("", 0, true).unwrap_err();
        assert!(matches!(err, CommandParseError::TooFewArguments { expected: 1, found: 0 }), "{err}");
    }

    #[test]
    fn custom_errors_keep_parse_errors_as_they_are() {
        let err = CommandParseError::custom(CommandParseError::TooFewArguments { expected: 1, found: 0 });
        assert!(matches!(err, CommandParseError::TooFewArguments { expected: 1, found: 0 }));
        let err = CommandParseError::custom("bad duration");
        assert!(matches!(err, CommandParseError::Custom(_)));
        assert_eq!(err.to_string(), "bad duration");
        let err = CommandParseError::custom("x".parse::<u8>().unwrap_err());
        assert!(err.source().is_some_and(|e| e.is::<std::num::ParseIntError>()));
    }

    /// A set whose parse panics, as a buggy hand-written or custom parser might.
    struct Panics;

    impl BotCommands for Panics {
        fn parse(_: &str, _: Option<&str>) -> Result<Self, CommandParseError> {
            panic!("parser bug")
        }

        fn filter<T: HasText>() -> Filter<T> {
            filters::command("x")
        }

        fn bot_commands() -> Vec<BotCommand> {
            Vec::new()
        }

        fn descriptions() -> CommandDescriptions<'static> {
            CommandDescriptions::new(&[])
        }
    }

    #[test]
    fn panicking_parse_leaves_me_unpoisoned() {
        let client = Client::offline();
        let raw = Message::request(7, "/x".into(), None);
        let message = Message::parse(&client, Box::new(raw), &Entities::default());
        let parse = std::panic::AssertUnwindSafe(|| message.parse_command::<Panics>());
        assert!(std::panic::catch_unwind(parse).is_err());
        assert!(!client.0.me.is_poisoned());
    }
}
