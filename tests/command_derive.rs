//! `#[derive(BotCommands)]` on sample enums, parsing the command text users send.
#![cfg(feature = "macros")]

use std::time::Duration;

use vaegram::commands::CommandParseError;
use vaegram::prelude::*;
use vaegram::test::{TestBuilder, TestMessage};
use vaegram::types::BotCommandScope;

/// The bot's commands. This doc comment is rustdoc only; the help header is `description`.
#[derive(BotCommands, Debug, PartialEq)]
#[command(description = "Commands:")]
enum Cmd {
    /// Start the bot
    Start(Option<String>),
    /// Ban a user, with an optional reason
    ///
    /// A second paragraph is a note for developers and stays out of the menu.
    #[command(alias = "b")]
    Ban {
        user_id: i64,
        #[command(rest)]
        reason: Option<String>,
    },
    /// Repeat the text
    Echo(String),
    /// Mute a user for
    /// some minutes
    Mute(i64, Option<u32>),
    /// Set your age
    SetAge(u8),
    /// Roll dice, like 2d6
    #[command(parse_with = dice, aliases = ["r", "dice"], hide_aliases)]
    Roll(u8, u8),
    /// Shout a word
    #[command(parse_with = shout)]
    Shout(String),
    /// Check the bot
    #[command(ephemeral)]
    Status,
    /// Show this help
    #[command(allow_extra, rename = "about")]
    Help,
    #[command(hide_menu)]
    Stats,
    #[command(hide)]
    Debug { verbose: Option<bool> },
}

/// A custom parser failing with plain text.
fn dice(text: &str) -> Result<(u8, u8), String> {
    let (count, sides) = text.split_once('d').ok_or("write it as NdM, like 2d6")?;
    Ok((count.parse().map_err(|_| "bad count")?, sides.parse().map_err(|_| "bad sides")?))
}

/// A custom parser failing with the runtime's own error.
fn shout(text: &str) -> Result<String, CommandParseError> {
    match text {
        "" => Err(CommandParseError::TooFewArguments { expected: 1, found: 0 }),
        _ => Ok(text.to_uppercase()),
    }
}

/// Commands for admins: two prefixes, exact case, and no menu.
#[derive(BotCommands, Debug, PartialEq)]
#[command(prefixes = ["/", "!"], case_sensitive, rename_rule = "lowercase", hide_menu)]
enum Admin {
    Kick(i64),
    SetMode,
}

/// A command with the longest name the derive takes, which the runtime must read too.
#[derive(BotCommands, Debug, PartialEq)]
#[command(hide_menu)]
enum Long {
    #[command(rename = "abcdefghijklmnopqrstuvwxyz_12345")]
    Longest,
}

fn parse(text: &str) -> Result<Cmd, CommandParseError> {
    Cmd::parse(text, Some("my_bot"))
}

fn error(text: &str) -> String {
    parse(text).unwrap_err().to_string()
}

#[test]
fn parses_commands_and_their_arguments() {
    assert_eq!(parse("/start").unwrap(), Cmd::Start(None));
    assert_eq!(parse("/start ref_42").unwrap(), Cmd::Start(Some("ref_42".into())));
    assert_eq!(parse("/ban 42").unwrap(), Cmd::Ban { user_id: 42, reason: None });
    assert_eq!(parse("/ban  42   spam  bot ").unwrap(), Cmd::Ban { user_id: 42, reason: Some("spam  bot".into()) });
    assert_eq!(parse("/b 7 flood").unwrap(), Cmd::Ban { user_id: 7, reason: Some("flood".into()) });
    assert_eq!(parse("/echo hello world").unwrap(), Cmd::Echo("hello world".into()));
    assert_eq!(parse("/echo   hello   world  ").unwrap(), Cmd::Echo("hello   world".into()));
    // One field reads all the text, as a hand-written impl does with `parse_opt_rest`.
    assert_eq!(parse("/start a b ").unwrap(), Cmd::Start(Some("a b".into())));
    assert_eq!(parse("/mute 5").unwrap(), Cmd::Mute(5, None));
    assert_eq!(parse("/mute 5 30").unwrap(), Cmd::Mute(5, Some(30)));
    assert_eq!(parse("/set_age 30 ").unwrap(), Cmd::SetAge(30));
    assert_eq!(parse("/status").unwrap(), Cmd::Status);
    assert_eq!(parse("/about anything at all").unwrap(), Cmd::Help);
    assert_eq!(parse("/stats").unwrap(), Cmd::Stats);
    assert_eq!(parse("/debug").unwrap(), Cmd::Debug { verbose: None });
    assert_eq!(parse("/debug true").unwrap(), Cmd::Debug { verbose: Some(true) });
}

#[test]
fn names_match_in_any_case_with_this_bots_mention() {
    assert_eq!(parse("/SET_AGE 30").unwrap(), Cmd::SetAge(30));
    assert_eq!(parse("/Ban@My_Bot 1").unwrap(), Cmd::Ban { user_id: 1, reason: None });
    assert!(matches!(parse("/ban@other_bot 1"), Err(CommandParseError::WrongBot(bot)) if bot == "other_bot"));
    // Before the client knows its username, a bare command parses and any mention is refused.
    assert_eq!(Cmd::parse("/status", None).unwrap(), Cmd::Status);
    assert!(matches!(Cmd::parse("/status@my_bot", None), Err(CommandParseError::WrongBot(_))));
}

#[test]
fn renames_and_aliases_replace_the_variant_name() {
    assert!(matches!(parse("/help"), Err(CommandParseError::UnknownCommand(name)) if name == "help"));
    assert!(matches!(parse("/setage 30"), Err(CommandParseError::UnknownCommand(_))));
    assert_eq!(parse("/r 1d20").unwrap(), Cmd::Roll(1, 20));
    assert_eq!(parse("/dice 3d4").unwrap(), Cmd::Roll(3, 4));
}

#[test]
fn wrong_arguments_name_the_field() {
    assert!(matches!(parse("/ban"), Err(CommandParseError::TooFewArguments { expected: 1, found: 0 })));
    assert!(matches!(parse("/ban abc"), Err(CommandParseError::InvalidArgument { index: 0, field: "user_id", .. })));
    assert!(matches!(parse("/mute 1 2 3"), Err(CommandParseError::TooManyArguments { expected: 2, found: 3 })));
    assert!(matches!(
        parse("/set_age 1 2"),
        Err(CommandParseError::InvalidArgument { index: 0, field: "argument", .. })
    ));
    assert!(matches!(parse("/status now"), Err(CommandParseError::TooManyArguments { expected: 0, found: 1 })));
    assert_eq!(error("/echo"), "expected at least 1 argument, got 0");
    assert_eq!(error("/set_age"), "expected at least 1 argument, got 0");
    assert_eq!(error("/set_age 300"), "invalid argument: number too large to fit in target type");
    assert_eq!(error("/mute x"), "invalid argument 1: invalid digit found in string");
    assert_eq!(error("/mute 1 x"), "invalid argument 2: invalid digit found in string");
    assert_eq!(error("/nope"), "unknown command nope");
    assert_eq!(error("hello"), "not a command");
}

#[test]
fn custom_parsers_get_all_the_text() {
    assert_eq!(parse("/roll 2d6").unwrap(), Cmd::Roll(2, 6));
    // A trailing space, as phone keyboards add, isn't part of the parser's text.
    assert_eq!(parse("/roll 2d6 ").unwrap(), Cmd::Roll(2, 6));
    assert!(matches!(parse("/roll two"), Err(CommandParseError::Custom(_))));
    assert_eq!(error("/roll two"), "write it as NdM, like 2d6");
    assert_eq!(error("/roll xd6"), "bad count");
    assert_eq!(parse("/shout hi there ").unwrap(), Cmd::Shout("HI THERE".into()));
    // The parser's own `CommandParseError` comes out as itself, not wrapped in `Custom`.
    assert!(matches!(parse("/shout"), Err(CommandParseError::TooFewArguments { expected: 1, found: 0 })));
}

#[test]
fn the_longest_name_the_derive_takes_parses() {
    assert_eq!(Long::parse("/abcdefghijklmnopqrstuvwxyz_12345", None).unwrap(), Long::Longest);
    assert_eq!(Long::descriptions().to_string(), "/abcdefghijklmnopqrstuvwxyz_12345");
}

#[test]
fn menu_lists_documented_commands_only() {
    let menu = Cmd::bot_commands();
    let entries: Vec<(&str, &str, bool)> =
        menu.iter().map(|c| (c.command.as_str(), c.description.as_str(), c.is_ephemeral)).collect();
    assert_eq!(
        entries,
        [
            ("start", "Start the bot", false),
            ("ban", "Ban a user, with an optional reason", false),
            ("echo", "Repeat the text", false),
            ("mute", "Mute a user for some minutes", false),
            ("set_age", "Set your age", false),
            ("roll", "Roll dice, like 2d6", false),
            ("shout", "Shout a word", false),
            ("status", "Check the bot", true),
            ("about", "Show this help", false),
        ]
    );
    assert_eq!(Cmd::bot_commands_for(&BotCommandScope::AllPrivateChats), menu);
    assert!(Admin::bot_commands().is_empty());
}

#[test]
fn help_text_lists_shown_commands() {
    assert_eq!(
        Cmd::descriptions().to_string(),
        "Commands:\n\n/start - Start the bot\n/ban, /b - Ban a user, with an optional reason\n/echo - Repeat the text\n\
         /mute - Mute a user for some minutes\n/set_age - Set your age\n/roll - Roll dice, like 2d6\n\
         /shout - Shout a word\n/status - Check the bot\n/about - Show this help\n/stats"
    );
    assert_eq!(Admin::descriptions().to_string(), "/kick\n/setmode");
}

#[test]
fn prefixes_and_exact_case() {
    let admin = |text| Admin::parse(text, None);
    assert_eq!(admin("!kick 5").unwrap(), Admin::Kick(5));
    assert_eq!(admin("/setmode").unwrap(), Admin::SetMode);
    assert!(matches!(admin("/Kick 5"), Err(CommandParseError::UnknownCommand(name)) if name == "Kick"));
    assert!(matches!(admin(".kick 5"), Err(CommandParseError::NotACommand)));
}

/// Runs `f` and fails if it takes 5 seconds: a test client answers at once.
async fn soon<T>(f: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), f).await.expect("it hung")
}

#[tokio::test]
async fn the_filter_lets_through_what_parse_reads() {
    let (client, recorder) = TestBuilder::new().bot(7, Some("my_bot")).recording();
    client.on_message(Cmd::filter(), |msg| async move {
        let text = match msg.parse_command::<Cmd>() {
            Ok(cmd) => format!("{cmd:?}"),
            Err(err) => format!("error: {err}"),
        };
        msg.reply(text).await
    });
    client.on_message(Admin::filter(), |msg| async move {
        let cmd = msg.parse_command::<Admin>()?;
        msg.reply(format!("admin {cmd:?}")).await
    });
    for text in [
        "/ban@my_bot 42 spam bot",
        "/B 7",
        "/ban@other_bot 1",
        "hello",
        "/debug",
        "/ban x",
        "/nope",
        "!kick 9",
        "!Kick 9",
    ] {
        soon(recorder.feed(TestMessage::private(42, text))).await;
    }
    assert_eq!(
        recorder.sent_texts(42),
        [
            "Ban { user_id: 42, reason: Some(\"spam bot\") }",
            "Ban { user_id: 7, reason: None }",
            "Debug { verbose: None }",
            "error: invalid user_id: invalid digit found in string",
            "admin Kick(9)",
        ]
    );
}
