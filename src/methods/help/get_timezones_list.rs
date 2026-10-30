use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Timezone;

impl Client {
    pub async fn get_timezones_list(&self) -> Result<Vec<Timezone>> {
        let list = match self.invoke(&functions::help::GetTimezonesList { hash: 0 }).await? {
            base::help::TimezonesList::TimezonesList(list) => list,
            base::help::TimezonesList::TimezonesListNotModified => return Ok(Vec::new()),
        };
        let timezones = list.timezones.into_iter().map(|base::Timezone::Timezone(t)| Timezone {
            id: t.id,
            name: t.name,
            utc_offset_seconds: t.utc_offset,
        });
        Ok(timezones.collect())
    }
}
