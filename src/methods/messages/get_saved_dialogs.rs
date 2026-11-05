use crate::Client;
use crate::errors::Result;
use crate::methods::pages::DialogPages;
use crate::raw::functions;
use crate::types::SavedDialog;

/// The dialogs of Saved Messages. Call [`next`](GetSavedDialogs::next) until it returns `None`.
pub struct GetSavedDialogs<'a> {
    client: &'a Client,
    pages: DialogPages<SavedDialog>,
}

impl Client {
    /// Lists the dialogs Saved Messages is split into, one per original chat or sender.
    pub fn get_saved_dialogs(&self) -> GetSavedDialogs<'_> {
        GetSavedDialogs { client: self, pages: DialogPages::new() }
    }
}

impl GetSavedDialogs<'_> {
    /// Stops after this many dialogs. Without it the walk runs to the last dialog.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next dialog, fetching a page when the buffered one is used up. `None` after the last.
    pub async fn next(&mut self) -> Result<Option<SavedDialog>> {
        loop {
            if let Some(dialog) = self.pages.pop() {
                return Ok(Some(dialog));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = functions::messages::GetSavedDialogs {
                exclude_pinned: false,
                parent_peer: None,
                offset_date: self.pages.offset_date,
                offset_id: self.pages.offset_id,
                offset_peer: self.pages.offset_peer.clone(),
                limit,
                hash: 0,
            };
            let (dialogs, count, next) = SavedDialog::unpack(self.client, self.client.invoke(&request).await?);
            self.pages.refill(self.client, dialogs, count < limit as usize, next).await?;
        }
    }
}
