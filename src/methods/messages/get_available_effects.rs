use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::AvailableEffect;

impl Client {
    pub async fn get_available_effects(&self) -> Result<Vec<AvailableEffect>> {
        match self.invoke(&functions::messages::GetAvailableEffects { hash: 0 }).await? {
            base::messages::AvailableEffects::AvailableEffects(e) => {
                Ok(e.effects.into_iter().map(AvailableEffect::parse).collect())
            }
            base::messages::AvailableEffects::AvailableEffectsNotModified => Ok(Vec::new()),
        }
    }
}
