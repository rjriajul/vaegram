/// Whether a request wants updates to keep coming, by its TL name such as `messages.sendMessage`. A client built
/// with `auto_no_updates` sends the ones that don't inside `invokeWithoutUpdates`, so a connection that only
/// reads stays quiet.
///
/// Calls without a namespace, which are MTProto's own, and everything under `updates.` need them. The others
/// don't when the method starts with `Get`, `Check`, `Read` or `Ping`, contains `SetTyping` or
/// `SendScreenshot`, starts with `Upload` without `Profile` in it (each read as in `messages.GetHistory`), or is
/// one of a few searches and device and file calls.
pub(crate) fn needs_updates(name: &str) -> bool {
    let Some((namespace, method)) = name.split_once('.') else { return true };
    if namespace == "updates" {
        return true;
    }
    if matches!(
        name,
        "account.registerDevice"
            | "account.unregisterDevice"
            | "channels.searchPosts"
            | "messages.search"
            | "messages.searchCustomEmoji"
            | "messages.searchGlobal"
            | "messages.searchSentMedia"
            | "messages.searchEmojiStickerSets"
            | "messages.searchStickerSets"
            | "messages.searchStickers"
            | "stickers.suggestShortName"
            | "upload.reuploadCdnFile"
            | "upload.saveBigFilePart"
            | "upload.saveFilePart"
    ) {
        return false;
    }
    let reads = ["Get", "Check", "Read", "Ping"].into_iter().any(|word| starts(method, word));
    let chatter = has(method, "SetTyping") || has(method, "SendScreenshot");
    let upload = starts(method, "Upload") && !has(method, "Profile");
    !(reads || chatter || upload)
}

// The method read as PascalCase (`getHistory` as `GetHistory`), without making a copy of it.
fn starts(method: &str, word: &str) -> bool {
    let (mut method, mut word) = (method.chars(), word.chars());
    match (method.next(), word.next()) {
        (Some(m), Some(w)) if m.to_ascii_uppercase() == w => method.as_str().starts_with(word.as_str()),
        _ => false,
    }
}

pub(super) fn has(method: &str, word: &str) -> bool {
    starts(method, word) || method.chars().next().is_some_and(|first| method[first.len_utf8()..].contains(word))
}

#[cfg(test)]
mod tests {
    use super::needs_updates;
    use crate::raw::{RemoteCall, functions};

    // The requests the reference client sends without updates, in TL case.
    const QUIET: [&str; 104] = [
        "account.checkUsername",
        "account.getPassword",
        "account.getPrivacy",
        "account.getWallPapers",
        "account.registerDevice",
        "account.unregisterDevice",
        "channels.checkUsername",
        "channels.getAdminLog",
        "channels.getChannels",
        "channels.getGroupsForDiscussion",
        "channels.getParticipants",
        "channels.searchPosts",
        "channels.readHistory",
        "channels.readMessageContents",
        "contacts.getContacts",
        "contacts.getTopPeers",
        "help.getAppConfig",
        "help.getConfig",
        "help.getNearestDc",
        "help.getTermsOfService",
        "langpack.getLangPack",
        "langpack.getLanguages",
        "messages.getAllChats",
        "messages.getAllDrafts",
        "messages.getAllStickers",
        "messages.getAttachMenuBots",
        "messages.getAvailableEffects",
        "messages.getBotCallbackAnswer",
        "messages.getChatInviteImporters",
        "messages.getChats",
        "messages.getCommonChats",
        "messages.getDefaultTagReactions",
        "messages.getDialogs",
        "messages.getEmojiKeywords",
        "messages.getEmojiStickerGroups",
        "messages.getEmojiStatusGroups",
        "messages.getExtendedMedia",
        "messages.getFavedStickers",
        "messages.getFeaturedEmojiStickers",
        "messages.getFeaturedStickers",
        "messages.getForumTopics",
        "messages.getFullChat",
        "messages.getGameHighScores",
        "messages.getHistory",
        "messages.getMaskStickers",
        "messages.getMessageReactionsList",
        "messages.getMessages",
        "messages.getMessagesViews",
        "messages.getOldFeaturedStickers",
        "messages.getPeerDialogs",
        "messages.getPeerSettings",
        "messages.getPinnedDialogs",
        "messages.getPinnedSavedDialogs",
        "messages.getPollResults",
        "messages.getPollVotes",
        "messages.getRecentReactions",
        "messages.getRecentStickers",
        "messages.getReplies",
        "messages.getSavedDialogs",
        "messages.getSavedGifs",
        "messages.getSavedHistory",
        "messages.getSavedReactionTags",
        "messages.getScheduledHistory",
        "messages.getScheduledMessages",
        "messages.getSearchCounters",
        "messages.getSplitRanges",
        "messages.getStickerSet",
        "messages.getStickers",
        "messages.getSuggestedDialogFilters",
        "messages.getTopReactions",
        "messages.getUnreadMentions",
        "messages.getUnreadReactions",
        "messages.getWebPage",
        "messages.getWebPagePreview",
        "messages.checkHistoryImport",
        "messages.checkQuickReplyShortcut",
        "messages.getQuickReplies",
        "messages.getQuickReplyMessages",
        "messages.getMessagesFiltered",
        "messages.getMessagesFilteredOrMin",
        "messages.getMyStickers",
        "messages.getCustomEmojiDocuments",
        "messages.getDocumentInfo",
        "messages.search",
        "messages.searchCustomEmoji",
        "messages.searchGlobal",
        "messages.searchSentMedia",
        "messages.searchEmojiStickerSets",
        "messages.searchStickerSets",
        "messages.searchStickers",
        "messages.sendScreenshotNotification",
        "messages.setTyping",
        "stickers.checkShortName",
        "stickers.suggestShortName",
        "upload.getFile",
        "upload.getWebFile",
        "upload.reuploadCdnFile",
        "upload.saveBigFilePart",
        "upload.saveFilePart",
        "users.getFullUser",
        "users.getUsers",
        "premium.getMyBoosts",
        "premium.getBoostsList",
        "premium.getUserBoosts",
    ];

    #[test]
    fn the_rule_covers_every_reference_name() {
        for name in QUIET {
            assert!(!needs_updates(name), "{name}");
        }
    }

    #[test]
    fn writes_and_updates_need_updates() {
        for name in [
            "messages.sendMessage",
            "updates.getDifference",
            "updates.getState",
            "photos.uploadProfilePhoto",
            "channels.joinChannel",
            "messages.deleteMessages",
            "account.updateProfile",
            "messages.forwardMessages",
            "ping",
            "invokeWithLayer",
        ] {
            assert!(needs_updates(name), "{name}");
        }
        // Reads and the like don't, and reading the method as PascalCase is all that matters.
        for name in ["help.getConfig", "users.getUsers", "messages.setTyping", "messages.sendScreenshotNotification"] {
            assert!(!needs_updates(name), "{name}");
        }
        assert!(!needs_updates("messages.uploadMedia"), "starts with Upload");
        assert_eq!(needs_updates("messages.getHistory"), needs_updates("messages.GetHistory"));
    }

    #[test]
    fn table_names_are_generated_names() {
        let generated = [
            functions::account::RegisterDevice::NAME,
            functions::account::UnregisterDevice::NAME,
            functions::channels::SearchPosts::NAME,
            functions::messages::Search::NAME,
            functions::messages::SearchCustomEmoji::NAME,
            functions::messages::SearchGlobal::NAME,
            functions::messages::SearchSentMedia::NAME,
            functions::messages::SearchEmojiStickerSets::NAME,
            functions::messages::SearchStickerSets::NAME,
            functions::messages::SearchStickers::NAME,
            functions::stickers::SuggestShortName::NAME,
            functions::upload::ReuploadCdnFile::NAME,
            functions::upload::SaveBigFilePart::NAME,
            functions::upload::SaveFilePart::NAME,
        ];
        // Each of these is quiet by its name alone, not by a word in it.
        for name in generated {
            assert!(QUIET.contains(&name), "{name} isn't a reference name");
            assert!(!needs_updates(name), "{name}");
        }
    }
}
