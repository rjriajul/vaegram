use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ReactionType;

impl Client {
    /// Returns the reactions Telegram suggests as tags in Saved Messages.
    pub async fn get_default_tag_reactions(&self) -> Result<Vec<ReactionType>> {
        match self.invoke(&functions::messages::GetDefaultTagReactions { hash: 0 }).await? {
            base::messages::Reactions::Reactions(r) => Ok(r.reactions.iter().filter_map(ReactionType::parse).collect()),
            base::messages::Reactions::ReactionsNotModified => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::ReactionType;

    #[tokio::test]
    async fn lists_the_reactions_and_an_unchanged_answer_is_empty() {
        let like = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
        let custom = base::Reaction::ReactionCustomEmoji(types::ReactionCustomEmoji { document_id: 7 });
        let list = vec![like, base::Reaction::ReactionEmpty, custom];
        let reactions = types::messages::Reactions { hash: 3, reactions: list };
        let found = base::messages::Reactions::Reactions(Box::new(reactions));
        let unchanged = base::messages::Reactions::ReactionsNotModified;
        let (client, mut sent) = Client::scripted(vec![Ok(found.to_bytes()), Ok(unchanged.to_bytes())]);
        let tags = client.get_default_tag_reactions().await.unwrap();
        assert_eq!(tags, [ReactionType::Emoji("👍".into()), ReactionType::CustomEmoji(7)]);
        assert!(client.get_default_tag_reactions().await.unwrap().is_empty());
        for _ in 0..2 {
            assert_eq!(sent.try_recv().unwrap(), functions::messages::GetDefaultTagReactions { hash: 0 }.to_bytes());
        }
    }
}
