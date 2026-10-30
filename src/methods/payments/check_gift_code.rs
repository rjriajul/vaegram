use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::CheckedGiftCode;

impl Client {
    pub async fn check_gift_code(&self, slug: &str) -> Result<CheckedGiftCode> {
        let slug = gift_code_slug(slug)?.into();
        let base::payments::CheckedGiftCode::CheckedGiftCode(code) =
            self.invoke(&functions::payments::CheckGiftCode { slug }).await?;
        let mut code = *code;
        let entities = Entities::new(self, std::mem::take(&mut code.users), std::mem::take(&mut code.chats));
        Ok(CheckedGiftCode::parse(&code, &entities))
    }
}

pub(super) fn gift_code_slug(link: &str) -> Result<&str> {
    let link = link.trim().trim_end_matches('/');
    let slug = link.rsplit_once(['/', '=']).map_or(link, |(_, slug)| slug);
    // The bare `t.me/giftcode` and `tg://giftcode` leave the word itself behind.
    if slug.is_empty() || slug == "giftcode" {
        return Err(Error::InvalidArgument("the gift code link has no slug"));
    }
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn links_without_a_slug_are_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        for link in ["", "  ", "https://t.me/giftcode/", "t.me/giftcode", "tg://giftcode?slug="] {
            assert!(matches!(client.check_gift_code(link).await, Err(Error::InvalidArgument(_))), "{link}");
            assert!(matches!(client.apply_gift_code(link).await, Err(Error::InvalidArgument(_))), "{link}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn every_link_form_sends_the_slug() {
        let links = [
            "abc",
            "t.me/giftcode/abc",
            "https://t.me/giftcode/abc/",
            " telegram.me/giftcode/abc ",
            "tg://giftcode?slug=abc",
        ];
        for link in links {
            let (client, mut rx) = Client::recording();
            let _ = client.apply_gift_code(link).await;
            let body = rx.try_recv().unwrap();
            assert_eq!(body[body.len() - 4..], [3, b'a', b'b', b'c'], "{link}");
        }
    }
}
