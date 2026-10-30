use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Language;

impl Client {
    pub async fn get_languages(&self, lang_pack: impl Into<String>) -> Result<Vec<Language>> {
        let languages = self.invoke(&functions::langpack::GetLanguages { lang_pack: lang_pack.into() }).await?;
        Ok(languages.into_iter().map(Language::parse).collect())
    }
}
