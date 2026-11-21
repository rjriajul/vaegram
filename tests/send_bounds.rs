//! The futures the friendly API returns stay `Send`, so `tokio::spawn` takes them. A std lock held across an
//! `.await` inside any of them stops this file from compiling.

use std::future::IntoFuture;

use vaegram::Client;
use vaegram::raw::functions::Ping;
use vaegram::types::{CallbackQuery, Chat, Message, Story, Upload, User};

fn send<T: Send>(_: T) {}

fn send_sync<T: Send + Sync>() {}

// Every friendly type holds a `Client`; at the default recursion limit, proving them `Send` must not walk it. Only a
// nightly build checks that depth (its `recursion_depth_exceeding_limit` lint); stable compiles these regardless.
#[allow(dead_code)]
fn friendly_types_are_send_and_sync() {
    send_sync::<Client>();
    send_sync::<Message>();
    send_sync::<Story>();
    send_sync::<CallbackQuery>();
    send_sync::<Chat>();
    send_sync::<User>();
}

#[allow(dead_code)]
fn spawned_calls_on_friendly_types(client: Client, message: Message) {
    drop(tokio::spawn(async move { message.delete().await }));
    drop(tokio::spawn(async move { client.download_media("file id").save("a.txt").await }));
}

#[allow(dead_code)]
fn public_futures_are_send(client: &Client) {
    send(client.run());
    send(Client::compose(std::slice::from_ref(client)).into_future());
    send(client.invoke(&Ping { ping_id: 0 }));
    send(client.send_message("me", "hi").into_future());
    send(client.listen::<Message>().into_future());
    send(client.listen::<CallbackQuery>().into_future());
    send(client.ask("me", "?").into_future());
    send(client.upload_file(Upload::bytes("a.txt", b"x")).into_future());
    send(client.download_media("file id").into_future());
    send(client.download_media("file id").save("a.txt"));
}

#[test]
fn public_futures_compile_as_send() {}
