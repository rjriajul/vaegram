use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::TermsOfService;

impl Client {
    pub async fn get_terms_of_service_update(&self) -> Result<Option<TermsOfService>> {
        let update = self.invoke(&functions::help::GetTermsOfServiceUpdate {}).await?;
        let base::help::TermsOfServiceUpdate::TermsOfServiceUpdate(u) = update else { return Ok(None) };
        Ok(Some(TermsOfService::parse(u.terms_of_service)))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::MessageEntityType;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn reads_and_accepts_new_terms() {
        let id = base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{\"id\":7}".into() }));
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 });
        let (text, entities) = ("Be nice".into(), vec![bold]);
        let tos =
            types::help::TermsOfService { popup: true, id: id.clone(), text, entities, min_age_confirm: Some(18) };
        let tos = base::help::TermsOfService::TermsOfService(Box::new(tos));
        let update = types::help::TermsOfServiceUpdate { expires: 1, terms_of_service: tos };
        let none = types::help::TermsOfServiceUpdateEmpty { expires: 1 };
        let (client, mut sent) = Client::scripted(vec![
            Ok(base::help::TermsOfServiceUpdate::TermsOfServiceUpdate(Box::new(update)).to_bytes()),
            Ok(base::help::TermsOfServiceUpdate::TermsOfServiceUpdateEmpty(none).to_bytes()),
            Ok(true.to_bytes()),
        ]);

        let tos = client.get_terms_of_service_update().await.unwrap().unwrap();
        let bold = tos.entities.as_deref().map(|e| (e.len(), e[0].kind, e[0].length));
        assert_eq!(
            (tos.id.as_str(), tos.text.as_str(), tos.popup, tos.min_age_confirm, bold),
            ("{\"id\":7}", "Be nice", true, Some(18), Some((1, MessageEntityType::Bold, 2)))
        );
        assert_eq!(client.get_terms_of_service_update().await.unwrap(), None);
        assert!(client.accept_terms_of_service(tos.id).await.unwrap());

        let request = functions::help::GetTermsOfServiceUpdate {}.to_bytes();
        assert_eq!((sent.recv().await.unwrap(), sent.recv().await.unwrap()), (request.clone(), request));
        assert_eq!(sent.recv().await.unwrap(), functions::help::AcceptTermsOfService { id }.to_bytes());
    }
}
