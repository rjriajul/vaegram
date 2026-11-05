use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::LangPackDifference;

impl Client {
    /// Returns every string of a language pack in `lang_code`, with the pack's version.
    pub async fn get_lang_pack(
        &self,
        lang_pack: impl Into<String>,
        lang_code: impl Into<String>,
    ) -> Result<LangPackDifference> {
        let request = functions::langpack::GetLangPack { lang_pack: lang_pack.into(), lang_code: lang_code.into() };
        Ok(LangPackDifference::parse(self.invoke(&request).await?))
    }
}
