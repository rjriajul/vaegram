use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::LangPackDifference;

impl Client {
    pub async fn get_lang_pack_difference(
        &self,
        lang_pack: impl Into<String>,
        lang_code: impl Into<String>,
        from_version: i32,
    ) -> Result<LangPackDifference> {
        let (lang_pack, lang_code) = (lang_pack.into(), lang_code.into());
        let request = functions::langpack::GetDifference { lang_pack, lang_code, from_version };
        Ok(LangPackDifference::parse(self.invoke(&request).await?))
    }
}
