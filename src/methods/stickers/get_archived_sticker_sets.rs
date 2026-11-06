use crate::Client;
use crate::enums::StickerType;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::StickerSet;

/// Archived sticker sets of one type, built by [`Client::get_archived_sticker_sets`].
///
/// Awaiting it collects the sets into a `Vec`, up to [`limit`](GetArchivedStickerSets::limit);
/// [`next`](GetArchivedStickerSets::next) reads them one at a time. Sets are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetArchivedStickerSets<'a> {
    client: &'a Client,
    request: functions::messages::GetArchivedStickers,
    pages: SetPages,
}

impl Client {
    /// Lists the archived sticker sets of one `sticker_type`.
    pub fn get_archived_sticker_sets(&self, sticker_type: StickerType) -> GetArchivedStickerSets<'_> {
        let request = functions::messages::GetArchivedStickers {
            masks: sticker_type == StickerType::Mask,
            emojis: sticker_type == StickerType::CustomEmoji,
            offset_id: 0,
            limit: 0,
        };
        GetArchivedStickerSets { client: self, request, pages: SetPages::new() }
    }
}

impl GetArchivedStickerSets<'_> {
    /// Starts after the set with this id instead of at the first set.
    pub fn offset_id(mut self, set_id: i64) -> Self {
        self.pages.sets.set_offset(set_id);
        self
    }

    /// Stops after this many sets in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.sets.remaining = limit;
        self
    }

    /// Returns the next set, or `None` once the sets or the limit run out.
    pub async fn next(&mut self) -> Result<Option<StickerSet>> {
        loop {
            if let Some(set) = self.pages.sets.pop() {
                return Ok(Some(set));
            }
            let Some(limit) = self.pages.sets.limit() else { return Ok(None) };
            (self.request.offset_id, self.request.limit) = (self.pages.sets.offset().copied().unwrap_or(0), limit);
            let base::messages::ArchivedStickers::ArchivedStickers(page) = self.client.invoke(&self.request).await?;
            self.pages.push(&page.sets, page.count);
        }
    }
}

pager!(GetArchivedStickerSets, StickerSet);

/// Pages of sticker sets that continue after the id of the last set read.
pub(super) struct SetPages {
    pub(super) sets: OffsetPages<StickerSet, i64>,
    read: usize,
}

impl SetPages {
    pub(super) fn new() -> Self {
        SetPages { sets: OffsetPages::starting_at(100, 0), read: 0 }
    }

    /// Buffers a page; `count` is how many sets the whole list holds.
    pub(super) fn push(&mut self, page: &[base::StickerSetCovered], count: i32) {
        let sets: Vec<_> = page.iter().map(StickerSet::parse_covered).collect();
        self.read += sets.len();
        // An empty page ends the list too: asking again would loop forever. `read` counts from where this pager
        // started, so after `offset_id` it stays below `count` and the list ends on that empty page, one request
        // later than a list read from the start.
        let next = sets.last().map(|set| set.id).filter(|_| self.read < usize::try_from(count).unwrap_or(0));
        self.sets.push(sets, next);
    }
}
