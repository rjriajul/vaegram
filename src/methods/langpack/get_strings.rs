use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::LangPackString;

impl Client {
    pub async fn get_strings(
        &self,
        lang_pack: impl Into<String>,
        lang_code: impl Into<String>,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Vec<LangPackString>> {
        let (lang_pack, lang_code) = (lang_pack.into(), lang_code.into());
        let keys: Vec<String> = keys.into_iter().map(Into::into).collect();
        if keys.is_empty() {
            return Err(Error::InvalidArgument("no string keys given"));
        }
        let strings = self.invoke(&functions::langpack::GetStrings { lang_pack, lang_code, keys }).await?;
        Ok(strings.into_iter().map(LangPackString::parse).collect())
    }
}
