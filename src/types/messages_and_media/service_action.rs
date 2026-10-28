use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::Community;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceAction {
    CommunityChatAdded(Community),
    CommunityChatRemoved,
    CommunityChatJoined(Community),
}

impl ServiceAction {
    pub(crate) fn parse(action: &base::MessageAction, entities: &Entities) -> Option<ServiceAction> {
        use base::MessageAction as A;
        Some(match action {
            A::MessageActionChangeCommunity(a) => match a.community_id {
                Some(id) => ServiceAction::CommunityChatAdded(Community::parse(id, entities)),
                // An absent community_id means the chat left its community.
                None => ServiceAction::CommunityChatRemoved,
            },
            A::MessageActionChatJoinedViaCommunity(a) => {
                ServiceAction::CommunityChatJoined(Community::parse(a.community_id, entities))
            }
            _ => return None,
        })
    }
}
