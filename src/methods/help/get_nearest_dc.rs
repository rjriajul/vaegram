use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::NearestDc;

impl Client {
    pub async fn get_nearest_dc(&self) -> Result<NearestDc> {
        let base::NearestDc::NearestDc(dc) = self.invoke(&functions::help::GetNearestDc {}).await?;
        Ok(NearestDc { country: dc.country, this_dc: dc.this_dc, nearest_dc: dc.nearest_dc })
    }
}
