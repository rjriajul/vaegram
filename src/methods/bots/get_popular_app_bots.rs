use crate::Client;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions, types};
use crate::types::User;

#[must_use = "call .next() to fetch bots"]
pub struct GetPopularAppBots<'a> {
    client: &'a Client,
    pages: OffsetPages<User>,
}

impl Client {
    pub fn get_popular_app_bots(&self) -> GetPopularAppBots<'_> {
        GetPopularAppBots { client: self, pages: OffsetPages::new(100) }
    }
}

impl GetPopularAppBots<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<User>> {
        loop {
            if let Some(bot) = self.pages.pop() {
                return Ok(Some(bot));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let offset = self.pages.next_offset().unwrap_or_default().into();
            let request = functions::bots::GetPopularAppBots { offset, limit };
            let base::bots::PopularAppBots::PopularAppBots(page) = self.client.invoke(&request).await?;
            let types::bots::PopularAppBots { next_offset, users } = *page;
            self.pages.refill(users, next_offset, |users| User::parse_all(self.client, users));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn stops_on_an_empty_page() {
        let page = |users, next: &str| {
            let page = types::bots::PopularAppBots { next_offset: Some(next.into()), users };
            Ok(base::bots::PopularAppBots::PopularAppBots(Box::new(page)).to_bytes())
        };
        let empty_user = base::User::UserEmpty(types::UserEmpty { id: 5 });
        let (client, mut sent) = Client::scripted(vec![page(vec![empty_user], "x"), page(Vec::new(), "y")]);

        let mut bots = client.get_popular_app_bots();
        assert_eq!(bots.next().await.unwrap(), None);
        let (first, second) = (sent.try_recv().unwrap(), sent.try_recv().unwrap());
        assert_eq!((&first[4..5], &second[4..6]), (&[0][..], &[1, b'x'][..]));
        assert!(sent.try_recv().is_err());
    }
}
