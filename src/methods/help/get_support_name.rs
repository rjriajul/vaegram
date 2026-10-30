use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn get_support_name(&self) -> Result<String> {
        let base::help::SupportName::SupportName(s) = self.invoke(&functions::help::GetSupportName {}).await?;
        Ok(s.name)
    }
}
