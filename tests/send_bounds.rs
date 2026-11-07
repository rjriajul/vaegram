//! The futures the friendly API returns stay `Send`, so `tokio::spawn` takes them. A std lock held across an
//! `.await` inside any of them stops this file from compiling.

use std::future::IntoFuture;

use vaegram::Client;
use vaegram::raw::functions::Ping;
use vaegram::types::{CallbackQuery, Message, Upload};

fn send<T: Send>(_: T) {}

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
