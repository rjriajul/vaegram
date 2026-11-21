use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::EmojiGroup;

impl Client {
    /// Returns the groups of the emoji picker, each with the emoji it holds.
    pub async fn get_emoji_groups(&self) -> Result<Vec<EmojiGroup>> {
        Ok(match self.invoke(&functions::messages::GetEmojiGroups { hash: 0 }).await? {
            base::messages::EmojiGroups::EmojiGroups(r) => r.groups.iter().map(EmojiGroup::parse).collect(),
            base::messages::EmojiGroups::EmojiGroupsNotModified => Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn lists_the_groups_and_an_unchanged_answer_is_empty() {
        let animals = types::EmojiGroup { title: "Animals".into(), icon_emoji_id: 4, emoticons: vec!["🐱".into()] };
        let premium = types::EmojiGroupPremium { title: "Premium".into(), icon_emoji_id: 5 };
        let groups = vec![
            base::EmojiGroup::EmojiGroup(Box::new(animals)),
            base::EmojiGroup::EmojiGroupPremium(Box::new(premium)),
        ];
        let list = types::messages::EmojiGroups { hash: 9, groups };
        let found = base::messages::EmojiGroups::EmojiGroups(Box::new(list));
        let unchanged = base::messages::EmojiGroups::EmojiGroupsNotModified;
        let (client, mut sent) = Client::scripted(vec![Ok(found.to_bytes()), Ok(unchanged.to_bytes())]);
        let groups = client.get_emoji_groups().await.unwrap();
        let titles: Vec<_> = groups.iter().map(|g| (g.title.as_str(), g.is_premium)).collect();
        assert_eq!(titles, [("Animals", false), ("Premium", true)]);
        assert_eq!(groups[0].emoticons, ["🐱"]);
        assert!(client.get_emoji_groups().await.unwrap().is_empty());
        for _ in 0..2 {
            assert_eq!(sent.try_recv().unwrap(), functions::messages::GetEmojiGroups { hash: 0 }.to_bytes());
        }
    }
}
