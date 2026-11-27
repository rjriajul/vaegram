//! A bot split into routers: an admin router behind an outer middleware, a moderation router nested in it, a
//! typing indicator around its handlers, and values provided to handlers.

use std::sync::Arc;
use std::time::Instant;

use vaegram::enums::ChatAction;
use vaegram::handlers::{Next, UpdateContext, chat_action};
use vaegram::prelude::*;

/// Shared state, provided once and taken by handlers as `Dep<Arc<Config>>`.
struct Config {
    admins: Vec<i64>,
}

/// Who the admin is, provided by the admin router's middleware for each update.
#[derive(Clone)]
struct Admin(i64);

async fn ban(msg: Message, Dep(admin): Dep<Admin>) -> Result<()> {
    msg.reply(format!("Banned on behalf of {}.", admin.0)).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app = Client::builder("routers_bot", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .bot_token(env("VAEGRAM_BOT_TOKEN"))
        .start()
        .await?;
    let admins = env("VAEGRAM_ADMINS").split(',').filter_map(|id| id.trim().parse().ok()).collect();
    app.provide(Arc::new(Config { admins }));

    let mut moderation = Router::new();
    moderation.inner_middleware(chat_action(ChatAction::Typing));
    moderation.message().handle(filters::command("ban"), ban);

    let mut admin = Router::new();
    admin.message().filter(filters::private());
    admin.outer_middleware(|mut ctx: UpdateContext, next: Next| async move {
        let config = ctx.data().get::<Arc<Config>>().cloned();
        let allowed = |id: i64| config.as_ref().is_some_and(|c| c.admins.contains(&id));
        match ctx.user_id() {
            Some(id) if allowed(id) => {
                ctx.provide(Admin(id));
                let started = Instant::now();
                let result = next.run(ctx).await;
                eprintln!("admin update handled in {:?}", started.elapsed());
                result
            }
            // Kept from this router; the handlers in the client's later groups still get it.
            _ => Ok(()),
        }
    });
    admin.on_error(|e| async move { eprintln!("admin handler failed: {}", e.error) });
    admin.include_router(moderation);
    app.include_router(admin).group(-1);

    app.on_message(filters::command("start"), |msg| async move { msg.reply("Welcome!").await });
    app.run().await
}
