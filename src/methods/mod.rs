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

pub(crate) use advanced::invoke_sleeping_floods;
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
pub use messages::{
    ApproveSuggestedPost, CopyMessage, CopyMessages, DeclineSuggestedPost, DownloadMedia, EditEphemeralMessage,
    EditInlineMedia, EditInlineText, EditMessageChecklist, EditMessageLiveLocation, EditMessageMedia,
    EditMessageReplyMarkup, EditMessageText, ForwardMessages, GetChatHistory, MediaStream, MessageStream,
    PinChatMessage, SearchMessages, SendAnimation, SendAudio, SendChatAction, SendChecklist, SendContact, SendDice,
    SendDocument, SendEphemeralMessage, SendLivePhoto, SendLocation, SendMediaGroup, SendMessage, SendMessageDraft,
    SendPaidMedia, SendPhoto, SendPoll, SendReaction, SendRichMessage, SendSticker, SendVenue, SendVideo,
    SendVideoNote, SendVoice, StopMessageLiveLocation, StopPoll, StreamMessage, UnpinChatMessage,
};
pub(crate) use messages::{StreamShared, caption_setters, geo_point, input_venue};
pub use payments::{
    ConvertGiftToStars, GetChatGifts, GetStarTransactions, GiftPremiumSubscription, SendGift, TransferGift, UpgradeGift,
};
pub use stickers::CreateNewStickerSet;
pub use users::{GetChatPhotos, GetUserProfileAudios, SetProfilePhoto};
