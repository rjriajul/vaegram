use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ProfileAccentColor;

impl Client {
    pub async fn get_peer_profile_colors(&self) -> Result<Vec<ProfileAccentColor>> {
        match self.invoke(&functions::help::GetPeerProfileColors { hash: 0 }).await? {
            base::help::PeerColors::PeerColors(c) => Ok(c.colors.into_iter().map(ProfileAccentColor::parse).collect()),
            base::help::PeerColors::PeerColorsNotModified => Ok(Vec::new()),
        }
    }
}
