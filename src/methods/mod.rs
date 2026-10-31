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
mod premium;
mod stickers;
mod stories;
mod users;
mod utilities;

pub use account::{CreateTheme, InstallTheme};
pub(crate) use advanced::invoke_sleeping_floods;
pub use bots::{
    AnswerCallbackQuery, AnswerInlineQuery, AnswerPreCheckoutQuery, AnswerShippingQuery, CreateInvoiceLink,
    GetInlineBotResults, GetPopularAppBots, RequestCallbackAnswer, SavePreparedInlineMessage, SendGame,
    SendInlineBotResult, SendInvoice, SetGameScore, SetInlineGameScore, SetUserEmojiStatus, ToggleBotInAttachMenu,
    VerifyChat,
};
pub use business::{
    CreateBusinessChatLink, EditBusinessChatLink, EditStory, PostStory, RepostStory, SetBusinessAwayMessage,
};
pub use chats::{
    BanChatMember, CreateForumTopic, DeleteChatHistory, DeleteChatWallpaper, EditForumTopic, GetChatAdministrators,
    GetChatEventLog, GetChatMembers, GetDialogs, GetForumTopics, GetMessagePublicForwards, RestrictChatMember,
    SetChatAccentColor, SetChatPhoto, SetChatWallpaper, UpdateChatNotifications,
};
pub use contacts::AddContact;
pub use folders::{CreateFolderInviteLink, EditFolderInviteLink, LeaveFolder};
pub use invite_links::{
    CreateChatInviteLink, CreateChatSubscriptionInviteLink, EditChatInviteLink, GetChatJoinRequests,
};
pub use messages::{
    ApproveSuggestedPost, CopyMessage, CopyMessages, DeclineSuggestedPost, DownloadMedia, EditEphemeralMessage,
    EditInlineMedia, EditInlineText, EditMessageChecklist, EditMessageLiveLocation, EditMessageMedia,
    EditMessageReplyMarkup, EditMessageText, ExportMessageLink, ForwardMessages, GetChatHistory, GetDiscussionReplies,
    GetMessageReactionsList, GetPollVotes, GetSavedDialogs, GetSavedHistory, MediaStream, MessageStream,
    PinChatMessage, RequestAppWebView, RequestMainWebView, RequestSimpleWebView, RequestWebView, SaveDraft,
    SearchGlobal, SearchMessages, SearchMessagesCount, SearchPosts, SendAnimation, SendAudio, SendChatAction,
    SendChecklist, SendContact, SendDice, SendDocument, SendEphemeralMessage, SendLivePhoto, SendLocation,
    SendMediaGroup, SendMessage, SendMessageDraft, SendPaidMedia, SendPaidReaction, SendPhoto, SendPoll, SendReaction,
    SendRichMessage, SendSticker, SendVenue, SendVideo, SendVideoNote, SendVoice, StopMessageLiveLocation, StopPoll,
    StreamMessage, TranslateText, UnpinChatMessage,
};
pub(crate) use messages::{DownloadSource, StreamShared, caption_setters, geo_point, input_venue};
pub use password::{ChangeCloudPassword, EnableCloudPassword};
pub use payments::{
    ConvertGiftToStars, GetChatGifts, GetStarTransactions, GiftPremiumSubscription, SearchGiftsForResale, SendGift,
    TransferGift, UpgradeGift,
};
pub use premium::GetBoostsList;
pub use stickers::{
    CreateNewStickerSet, GetArchivedStickerSets, GetFeaturedStickerSets, SearchStickerSets, SearchStickers,
};
pub use stories::{EditStoryCaption, EnableStealthMode, GetStoryViewers, SendStory, StoryPages};
pub use users::{GetChatPhotos, GetUserProfileAudios, SetEmojiStatus, SetProfilePhoto, UpdateProfile};
