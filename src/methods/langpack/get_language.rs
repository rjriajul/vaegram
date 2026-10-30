use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Language;

impl Client {
    pub async fn get_language(&self, lang_pack: impl Into<String>, lang_code: impl Into<String>) -> Result<Language> {
        let request = functions::langpack::GetLanguage { lang_pack: lang_pack.into(), lang_code: lang_code.into() };
        Ok(Language::parse(self.invoke(&request).await?))
    }
}
