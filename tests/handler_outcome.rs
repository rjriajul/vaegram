//! The handler bound is `Future<Output = Result<T>>`: each closure below must compile with no
//! annotations. Registering needs no connection, but building a `Client` does, so this only
//! has to type-check.

use vaegram::handlers::UpdateKind;
use vaegram::{Client, Error, Result, filters};

fn io() -> Result<()> {
    Ok(())
}

fn register(app: &Client) {
    app.on_message(filters::all(), |_| async { Ok(()) });
    app.on_message(filters::all(), |m| async move { m.reply("hi").await });
    app.on_message(filters::command("start"), |m| async move {
        io()?;
        m.reply("hi").await?;
        Ok(())
    })
    .group(-1)
    .stop();
    app.on_edited_message(filters::all(), |m| async move { m.delete().await }).group(2);
    app.on_callback_query(filters::all(), |q| async move { q.answer().await });
    app.on_raw_update(|_, _| async { Ok(1) });
    app.on_error(|e| async move {
        if e.kind == UpdateKind::Message {
            let _: (Option<i64>, Error) = (e.chat_id, e.error);
        }
    });
}

#[test]
fn closures_type_check() {
    let _ = register as fn(&Client);
}
