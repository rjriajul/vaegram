use super::create_business_chat_link::check_slug;
use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{BusinessChatLinkInfo, PeerRef};

impl Client {
    /// Takes the slug alone or a whole link: `t.me/m/<slug>`, `telegram.me/m/<slug>` or `tg://message?slug=<slug>`.
    pub async fn resolve_business_chat_link(&self, link: impl AsRef<str>) -> Result<BusinessChatLinkInfo> {
        let slug = slug_of(link.as_ref())?;
        check_slug(slug)?;
        let request = functions::account::ResolveBusinessChatLink { slug: slug.into() };
        let base::account::ResolvedBusinessChatLinks::ResolvedBusinessChatLinks(mut r) = self.invoke(&request).await?;
        let entities = Entities::new(self, std::mem::take(&mut r.users), std::mem::take(&mut r.chats));
        Ok(BusinessChatLinkInfo::parse(*r, &entities))
    }
}

fn slug_of(link: &str) -> Result<&str> {
    let link = link.trim();
    if let Some(slug) = PeerRef::tg_arg(link, "message", "slug") {
        return Ok(slug);
    }
    match PeerRef::link_path(link).map(|path| path.strip_prefix("m/")) {
        Some(Some(slug)) => Ok(slug.split('/').next().unwrap_or_default()),
        Some(None) => Err(Error::InvalidArgument("not a business chat link: those look like t.me/m/<slug>")),
        None => Ok(link),
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::Serializable;
    use crate::raw::functions::account::ResolveBusinessChatLink;

    #[tokio::test]
    async fn business_links_and_bare_slugs_send_one_slug() {
        let (client, mut rx) = Client::recording();
        let expected = ResolveBusinessChatLink { slug: "AbC1".into() }.to_bytes();
        for link in [
            "AbC1",
            " AbC1 ",
            "https://t.me/m/AbC1",
            "t.me/m/AbC1/",
            "http://telegram.me/m/AbC1?x=1",
            "telegram.dog/m/AbC1",
            "HTTPS://WWW.T.ME/m/AbC1",
            "tg://message?slug=AbC1",
            "tg:message?slug=AbC1&x=1",
            "TG://Message?slug=AbC1",
        ] {
            let _ = client.resolve_business_chat_link(link).await;
            assert_eq!(rx.try_recv().expect(link), expected, "{link}");
        }
        for refused in ["", "t.me/m/", "https://t.me/m/?x=1", "https://t.me/durov", "t.me/joinchat/AbC1", "t.me/"] {
            let result = client.resolve_business_chat_link(refused).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{refused}");
        }
        assert!(rx.try_recv().is_err());
    }
}
