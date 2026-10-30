use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::AccentColor;

impl Client {
    pub async fn get_peer_colors(&self) -> Result<Vec<AccentColor>> {
        match self.invoke(&functions::help::GetPeerColors { hash: 0 }).await? {
            base::help::PeerColors::PeerColors(c) => Ok(c.colors.into_iter().map(AccentColor::parse).collect()),
            base::help::PeerColors::PeerColorsNotModified => Ok(Vec::new()),
        }
    }
}
