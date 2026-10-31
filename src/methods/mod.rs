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
pub(crate) use into_future;

pub(crate) trait OnBehalf {
    fn business_connection(&mut self) -> &mut Option<String>;
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

pub use account::*;
pub(crate) use advanced::invoke_sleeping_floods;
pub use bots::*;
pub use business::*;
pub use chats::*;
pub use contacts::*;
pub use folders::*;
pub use invite_links::*;
pub use messages::*;
pub(crate) use messages::{DownloadSource, StreamShared, caption_setters, geo_point, input_venue};
pub use password::*;
pub use payments::*;
pub use phone::*;
pub use premium::*;
pub use stickers::*;
pub use stories::*;
pub use users::*;
