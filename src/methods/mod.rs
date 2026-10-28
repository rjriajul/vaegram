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

mod advanced;
mod auth;
mod bots;
mod business;
mod chats;
mod decorators;
mod invite_links;
mod messages;
mod payments;
mod stickers;
mod users;
mod utilities;

pub use bots::{
    AnswerCallbackQuery, AnswerInlineQuery, AnswerPreCheckoutQuery, AnswerShippingQuery, CreateInvoiceLink,
    SavePreparedInlineMessage, SendGame, SendInvoice, SetGameScore, SetInlineGameScore, SetUserEmojiStatus, VerifyChat,
};
pub use business::{EditStory, PostStory, RepostStory};
pub use chats::{
    BanChatMember, CreateForumTopic, EditForumTopic, GetChatAdministrators, GetChatMembers, RestrictChatMember,
    SetChatPhoto,
};
pub use invite_links::{
    CreateChatInviteLink, CreateChatSubscriptionInviteLink, EditChatInviteLink, GetChatJoinRequests,
};
pub(crate) use messages::StreamShared;
pub use messages::{
    ApproveSuggestedPost, CopyMessage, DeclineSuggestedPost, DownloadMedia, EditEphemeralMessage, EditInlineText,
    EditMessageChecklist, EditMessageReplyMarkup, EditMessageText, ForwardMessages, GetChatHistory, MessageStream,
    PinChatMessage, SearchMessages, SendChecklist, SendContact, SendDice, SendDocument, SendEphemeralMessage,
    SendLocation, SendMessage, SendMessageDraft, SendPhoto, SendPoll, SendReaction, SendRichMessage, SendVenue,
    StopPoll, StreamMessage, UnpinChatMessage,
};
pub use payments::{
    ConvertGiftToStars, GetChatGifts, GetStarTransactions, GiftPremiumSubscription, SendGift, TransferGift, UpgradeGift,
};
pub use stickers::CreateNewStickerSet;
pub use users::{GetChatPhotos, SetProfilePhoto};
