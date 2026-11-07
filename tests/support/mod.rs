use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::types::{Message, Text};
use vaegram::{Client, ClientBuilder, Error, Result, filters};

const REPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// A logged-in user and the test bot, both running. Dropping the kit disconnects both clients, clones included,
/// so keep it alive while any of them is in use.
pub struct Kit {
    pub user: User,
    pub bot: Client,
    _disconnect: Disconnect,
}

/// Disconnects the clients when dropped, also when the test panics, so no handler outlives its test.
pub struct Disconnect(pub Vec<Client>);

impl Drop for Disconnect {
    fn drop(&mut self) {
        for client in &self.0 {
            client.disconnect();
        }
    }
}

pub struct User {
    pub client: Client,
    received: tokio::sync::Mutex<mpsc::UnboundedReceiver<Message>>,
}

impl Kit {
    /// `None` (after saying so) when the env isn't set or the bot's login is flood-limited, so callers skip.
    pub async fn from_env() -> Option<Kit> {
        let vars = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_BOT_TOKEN"));
        let (Ok(session), Ok(token)) = vars else {
            eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name) and VAEGRAM_BOT_TOKEN");
            return None;
        };
        let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
        // Without catch-up, `run` starts from now: a saved session would otherwise get every update since it was
        // last used, and the bot's handlers would answer other tests' old messages.
        let user = Client::builder(session, api_id, api_hash.clone()).catch_up(false).connect().await.unwrap();
        let bot =
            start_bot(Client::builder("target/vaegram-kit-bot", api_id, api_hash).bot_token(token).catch_up(false))
                .await?;

        let (tx, received) = mpsc::unbounded_channel();
        user.on_message(filters::private(), move |m| {
            if !m.outgoing {
                let _ = tx.send(m);
            }
            async { Ok(()) }
        });
        for client in [&user, &bot] {
            let client = client.clone();
            tokio::spawn(async move { client.run().await });
        }
        let disconnect = Disconnect(vec![user.clone(), bot.clone()]);
        let user = User { client: user, received: tokio::sync::Mutex::new(received) };
        Some(Kit { user, bot, _disconnect: disconnect })
    }
}

/// Says so and returns `true` when `error` is a FLOOD_WAIT: Telegram is rate-limiting the account, so `what` can't
/// run now. Any other error is left to the caller.
pub fn flood_limited(error: &Error, what: &str) -> bool {
    let Some(wait) = error.flood_wait() else { return false };
    eprintln!("skipped: {what} is flood-limited for {}s", wait.as_secs());
    true
}

/// Says so and returns `true` when `error` is STORIES_TOO_MUCH: the account has posted as many stories as Telegram
/// allows it for now, far fewer without Premium.
#[allow(dead_code)] // not every binary that leaves the module's lints on posts stories
pub fn story_limit_reached(error: &Error) -> bool {
    if !error.is("STORIES_TOO_MUCH") {
        return false;
    }
    eprintln!("skipped: the account can't post another story for now (STORIES_TOO_MUCH)");
    true
}

/// Logs a bot in with `builder`, or says why not and returns `None` when Telegram flood-limits the login: a token
/// logging into new storage (`auth.importBotAuthorization`) too often waits for up to an hour.
pub async fn start_bot(builder: ClientBuilder) -> Option<Client> {
    match builder.start().await {
        Ok(bot) => Some(bot),
        Err(e) if flood_limited(&e, "logging the bot into new storage") => None,
        Err(e) => panic!("the bot didn't log in: {e:?}"),
    }
}

/// Checks a cleanup step that runs after the test's checks. A FLOOD_WAIT is only reported: what the test made
/// stays behind, and `what` names it so it can be removed by hand.
#[allow(dead_code)] // not every binary that leaves the module's lints on makes something to clean up
pub fn cleanup(result: Result<()>, what: &str) {
    let Err(e) = result else { return };
    match e.flood_wait() {
        Some(wait) => eprintln!("skipped the cleanup: {what} is flood-limited for {}s; do it by hand", wait.as_secs()),
        None => panic!("{what} failed: {e:?}"),
    }
}

/// Creates the supergroup "vaegram live test" for a test to work in, or says why not and returns `None` when
/// Telegram flood-limits it. The test deletes it through [`cleanup`].
#[allow(dead_code)] // not every binary that leaves the module's lints on makes a supergroup
pub async fn test_supergroup(user: &Client) -> Option<i64> {
    match user.create_supergroup("vaegram live test", "deleted right away").await {
        Ok(chat) => Some(chat.id),
        Err(e) if flood_limited(&e, "creating a supergroup") => None,
        Err(e) => panic!("creating a supergroup failed: {e:?}"),
    }
}

impl User {
    /// Sends `text` to `bot` in private and waits up to 30 s for the bot's answer there: a reply to the message sent.
    /// Every other message is another answer, or another test's traffic, and is dropped.
    pub async fn send_and_wait_reply(&self, bot: &Client, text: impl Into<Text>) -> Result<Message> {
        let me = bot.get_me().await?;
        let username = me.username.ok_or(Error::Unexpected("the bot has no username"))?;
        let bot_id = self.client.resolve_username(&username).await?;
        let mut received = self.received.lock().await;
        let sent = self.client.send_message(bot_id, text).await?.id;
        next_reply(&mut received, bot_id, sent).await
    }

    /// Waits up to 30 s for the bot's next reply to the user's message `to` in the bot's private chat `bot_id`, as
    /// [`send_and_wait_reply`](Self::send_and_wait_reply) does for its first one. The kit collects the bot's
    /// messages from its start, so a reply that came in before this call is still found.
    #[allow(dead_code)] // not every binary that leaves the module's lints on waits for a second reply
    pub async fn wait_reply(&self, bot_id: i64, to: i32) -> Result<Message> {
        next_reply(&mut *self.received.lock().await, bot_id, to).await
    }
}

async fn next_reply(received: &mut mpsc::UnboundedReceiver<Message>, bot_id: i64, to: i32) -> Result<Message> {
    tokio::time::timeout(REPLY_TIMEOUT, async {
        loop {
            match received.recv().await {
                Some(m) if m.chat.id == bot_id && m.id > to && m.reply_to_message_id == Some(to) => return Ok(m),
                Some(_) => {}
                None => return Err(Error::Disconnected),
            }
        }
    })
    .await
    .map_err(|_| Error::Timeout)?
}
