use crate::types::{FileSource, StickerSetRef};

/// The sticker [`Client::send_sticker`](crate::Client::send_sticker) sends: any [`FileSource`] (a [`Sticker`],
/// a path, a file id, …), or a sticker picked from a set by its id with [`from_set`](Self::from_set).
///
/// [`Sticker`]: crate::types::Sticker
#[derive(Debug, Clone)]
pub struct StickerSource<'a>(pub(crate) Kind<'a>);

#[derive(Debug, Clone)]
pub(crate) enum Kind<'a> {
    File(FileSource<'a>),
    InSet(StickerSetRef, i64),
}

impl StickerSource<'_> {
    /// The sticker with this `id` ([`Sticker::id`](crate::types::Sticker::id)) in `set`, by the set's short name or
    /// its [`StickerSetRef`].
    ///
    /// The client keeps the sets it sends from, so a send reads its set from Telegram only the first time, when the
    /// sticker isn't in the kept copy (then only if the set changed since), and once more when Telegram refuses the
    /// kept copy's file reference as expired, before sending again. A sticker that isn't in the set fails with
    /// [`Error::InvalidArgument`](crate::Error::InvalidArgument).
    pub fn from_set(set: impl Into<StickerSetRef>, id: i64) -> Self {
        StickerSource(Kind::InSet(set.into(), id))
    }
}

impl<'a, T: Into<FileSource<'a>>> From<T> for StickerSource<'a> {
    fn from(file: T) -> Self {
        StickerSource(Kind::File(file.into()))
    }
}
