use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Country;

impl Client {
    pub async fn get_countries_list(&self, lang_code: impl Into<String>) -> Result<Vec<Country>> {
        let request = functions::help::GetCountriesList { lang_code: lang_code.into(), hash: 0 };
        match self.invoke(&request).await? {
            base::help::CountriesList::CountriesList(l) => Ok(l.countries.into_iter().map(Country::parse).collect()),
            base::help::CountriesList::CountriesListNotModified => Ok(Vec::new()),
        }
    }
}
