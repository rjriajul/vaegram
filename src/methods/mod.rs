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

mod advanced;
mod auth;
mod bots;
mod chats;
mod decorators;
mod invite_links;
mod messages;
mod users;
mod utilities;

pub use bots::{AnswerCallbackQuery, AnswerInlineQuery, SavePreparedInlineMessage};
pub use chats::{BanChatMember, GetChatAdministrators, GetChatMembers, RestrictChatMember};
pub use invite_links::{
    CreateChatInviteLink, CreateChatSubscriptionInviteLink, EditChatInviteLink, GetChatJoinRequests,
};
pub(crate) use messages::StreamShared;
pub use messages::{
    ApproveSuggestedPost, CopyMessage, DeclineSuggestedPost, DownloadMedia, EditEphemeralMessage, EditMessageText,
    ForwardMessages, GetChatHistory, MessageStream, PinChatMessage, SearchMessages, SendContact, SendDice,
    SendDocument, SendEphemeralMessage, SendLocation, SendMessage, SendMessageDraft, SendPhoto, SendPoll, SendVenue,
    StopPoll, StreamMessage,
};
