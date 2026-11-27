//! The derive used from `vae` alone: with `crate = "vae"` the code it writes needs no `vaegram` dependency.
#![cfg(feature = "macros")]

use vae::commands::CommandParseError;
use vae::prelude::*;

#[derive(BotCommands, Debug, PartialEq)]
#[command(crate = "vae")]
enum Cmd {
    /// Start the bot
    Start,
    /// Ban a user
    Ban(i64),
}

#[test]
fn derive_names_the_vae_crate() {
    assert_eq!(Cmd::parse("/ban 42", None).unwrap(), Cmd::Ban(42));
    assert_eq!(Cmd::parse("/START", None).unwrap(), Cmd::Start);
    assert!(matches!(Cmd::parse("/kick", None), Err(CommandParseError::UnknownCommand(_))));
    let menu: Vec<String> = Cmd::bot_commands().into_iter().map(|c| c.command).collect();
    assert_eq!(menu, ["start", "ban"]);
}
