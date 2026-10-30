use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AttachMenuPeerType {
    SameBot,
    Bot,
    Private,
    Group,
    Channel,
}

impl AttachMenuPeerType {
    pub(crate) fn parse(kind: &base::AttachMenuPeerType) -> Self {
        use base::AttachMenuPeerType as T;
        match kind {
            T::AttachMenuPeerTypeSameBotPM => Self::SameBot,
            T::AttachMenuPeerTypeBotPM => Self::Bot,
            T::AttachMenuPeerTypePM => Self::Private,
            T::AttachMenuPeerTypeChat => Self::Group,
            T::AttachMenuPeerTypeBroadcast => Self::Channel,
        }
    }
}
