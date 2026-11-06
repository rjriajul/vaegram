macro_rules! into_future {
    ($builder:ident, $output:ty) => {
        impl<'a> ::std::future::IntoFuture for $builder<'a> {
            type Output = $crate::Result<$output>;
            type IntoFuture = ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Self::Output> + Send + 'a>>;

            fn into_future(self) -> Self::IntoFuture {
                Box::pin(self.send())
            }
        }
    };
}

/// Makes a pager awaitable: awaiting reads `next` until it returns `None` and collects what it yielded.
macro_rules! pager {
    ($pager:ident, $item:ty) => {
        impl<'a> ::std::future::IntoFuture for $pager<'a> {
            type Output = $crate::Result<Vec<$item>>;
            type IntoFuture = ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Self::Output> + Send + 'a>>;

            fn into_future(mut self) -> Self::IntoFuture {
                Box::pin(async move {
                    let mut items = Vec::new();
                    while let Some(item) = self.next().await? {
                        items.push(item);
                    }
                    Ok(items)
                })
            }
        }
    };
}

pub(crate) trait OnBehalf {
    fn business_connection(&mut self) -> &mut Option<String>;
}

/// The answer to an action Telegram confirms with `true`; `false` means it wasn't applied.
pub(crate) fn accepted(ok: bool) -> crate::Result<()> {
    if ok { Ok(()) } else { Err(crate::Error::Unexpected("Telegram answered false")) }
}

mod account;
mod advanced;
mod auth;
mod bots;
mod business;
mod chats;
mod contacts;
mod decorators;
mod folders;
mod help;
mod invite_links;
mod langpack;
mod listeners;
mod messages;
mod pages;
mod password;
mod payments;
mod phone;
mod premium;
mod stickers;
mod stories;
mod users;
mod utilities;
mod wallet;

pub use account::*;
pub(crate) use advanced::invoke_sleeping_floods;
pub use advanced::*;
pub use bots::*;
pub use business::*;
pub use chats::*;
pub use contacts::*;
pub use folders::*;
pub use invite_links::*;
pub use listeners::*;
pub use messages::*;
pub(crate) use messages::{DownloadSource, Routable, StreamShared, caption_setters, geo_point, input_venue};
pub use password::*;
pub use payments::*;
pub use phone::*;
pub use premium::*;
pub use stickers::*;
pub use stories::*;
pub use users::*;
pub use utilities::*;
pub use wallet::*;
