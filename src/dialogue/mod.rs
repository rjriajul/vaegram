//! Dialogues: multi-step conversations, such as a form that asks for a name, then an age, then a confirmation.
//!
//! A [`DialogueRouter`] holds the steps. Each step is a handler with an update filter and a [`StateFilter`] (from
//! [`filters`](crate::filters): [`state`](crate::filters::state), [`state_is`](crate::filters::state_is),
//! [`any_state`](crate::filters::any_state), [`no_state`](crate::filters::no_state)); it gets the update and a
//! [`Dialogue`] that reads and changes the conversation's state. The state is any `Clone` type of yours, usually an
//! enum with a variant per step that carries what the earlier steps collected. A [`DialogueStorage`] keeps it:
//! [`MemoryStorage`] in memory, `JsonFileStorage` (feature `dialogue-json`) in a file, or a backend of your own.
//! [`Scope`] picks whose conversation an update belongs to: by default each user's own in each chat.
//!
//! ```no_run
//! use std::time::Duration;
//! use vaegram::dialogue::{Dialogue, DialogueRouter, MemoryStorage};
//! use vaegram::prelude::*;
//!
//! #[derive(Clone)]
//! enum Signup {
//!     Name,
//!     Age { name: String },
//! }
//!
//! # fn f(client: Client) {
//! let mut signup = DialogueRouter::new("signup", MemoryStorage::new()).timeout(Duration::from_secs(600));
//! signup.on_message(filters::any_state(), filters::command("cancel"), |msg, dlg: Dialogue<Signup>| async move {
//!     dlg.clear().await?;
//!     msg.reply("Cancelled.").await
//! });
//! signup.on_message(filters::no_state(), filters::command("signup"), |msg, dlg| async move {
//!     dlg.set(Signup::Name).await?;
//!     msg.reply("What's your name?").await
//! });
//! let asked_name = filters::state(|s: &Signup| matches!(s, Signup::Name));
//! signup.on_message(asked_name, filters::text(), |msg, dlg| async move {
//!     dlg.set(Signup::Age { name: msg.text().to_owned() }).await?;
//!     msg.reply("How old are you?").await
//! });
//! let asked_age = filters::state(|s: &Signup| matches!(s, Signup::Age { .. }));
//! signup.on_message(asked_age, filters::text(), |msg, dlg| async move {
//!     let Some(Signup::Age { name }) = dlg.state() else { return Ok(()) };
//!     let Ok(age) = msg.text().trim().parse::<u8>() else {
//!         msg.reply("Send your age as a number.").await?;
//!         return Ok(());
//!     };
//!     dlg.clear().await?;
//!     msg.reply(format!("Welcome, {name} ({age})!")).await?;
//!     Ok(())
//! });
//! client.include_router(signup.into_router());
//! # }
//! ```
//!
//! # Dialogues and `ask`
//!
//! [`Client::ask`](crate::Client::ask) and `listen` wait inside one handler for the next reply: quick to write, but
//! the wait lives in memory, holds a handler slot, and ends with the process or its timeout. A dialogue keeps no
//! task waiting between messages, and its state lives in the storage, so with a persistent one a conversation
//! carries on after a restart. Use `ask` for a single quick question, a dialogue for a flow of several.

mod handle;
#[cfg(feature = "dialogue-json")]
mod json_file;
mod key;
mod locks;
mod memory;
mod router;
mod storage;

pub use handle::Dialogue;
#[cfg(feature = "dialogue-json")]
pub use json_file::JsonFileStorage;
pub use key::{DialogueKey, Scope};
pub use memory::MemoryStorage;
pub use router::DialogueRouter;
pub use storage::{DialogueStorage, Record};

pub use crate::filters::StateFilter;
