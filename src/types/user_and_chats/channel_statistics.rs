use crate::raw::base;
use crate::types::{StatisticalGraph, StatisticalValue};

/// Statistics of a channel over a period.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelStatistics {
    /// Unix time the period starts.
    pub min_date: i32,
    /// Unix time the period ends.
    pub max_date: i32,
    /// Follower count.
    pub followers: StatisticalValue,
    /// Average views of a post.
    pub views_per_post: StatisticalValue,
    /// Average shares of a post.
    pub shares_per_post: StatisticalValue,
    /// Average reactions on a post.
    pub reactions_per_post: StatisticalValue,
    /// Average views of a story.
    pub views_per_story: StatisticalValue,
    /// Average shares of a story.
    pub shares_per_story: StatisticalValue,
    /// Average reactions on a story.
    pub reactions_per_story: StatisticalValue,
    /// Percent of followers with notifications on, from 0 to 100. `0.0` if Telegram reports no followers.
    pub enabled_notifications: f64,
    /// Follower growth over time.
    pub growth_graph: StatisticalGraph,
    /// Followers over time.
    pub followers_graph: StatisticalGraph,
    /// Followers who muted the channel over time.
    pub mute_graph: StatisticalGraph,
    /// Views by hour of the day.
    pub top_hours_graph: StatisticalGraph,
    /// Post interactions over time.
    pub interactions_graph: StatisticalGraph,
    /// Instant View interactions over time.
    pub iv_interactions_graph: StatisticalGraph,
    /// Views by traffic source.
    pub views_by_source_graph: StatisticalGraph,
    /// New followers by traffic source.
    pub new_followers_by_source_graph: StatisticalGraph,
    /// Followers by language.
    pub languages_graph: StatisticalGraph,
    /// Post reactions by emoji.
    pub reactions_by_emotion_graph: StatisticalGraph,
    /// Story interactions over time.
    pub story_interactions_graph: StatisticalGraph,
    /// Story reactions by emoji.
    pub story_reactions_by_emotion_graph: StatisticalGraph,
    /// Interaction counts of recent posts and stories.
    pub recent_posts_interactions: Vec<PostInteractions>,
}

/// Interaction counts of one post or story.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostInteractions {
    /// Id of the message, or of the story if `is_story` is `true`.
    pub id: i32,
    /// `true` if this entry is a story instead of a message.
    pub is_story: bool,
    /// Views of the post or story.
    pub views: i32,
    /// Forwards of the post or story.
    pub forwards: i32,
    /// Reactions on the post or story.
    pub reactions: i32,
}

impl ChannelStatistics {
    pub(crate) fn parse(raw: base::stats::BroadcastStats) -> ChannelStatistics {
        let base::stats::BroadcastStats::BroadcastStats(s) = raw;
        let base::StatsDateRangeDays::StatsDateRangeDays(period) = s.period;
        let base::StatsPercentValue::StatsPercentValue(notifications) = s.enabled_notifications;
        let value = StatisticalValue::parse;
        let graph = StatisticalGraph::parse;
        ChannelStatistics {
            min_date: period.min_date,
            max_date: period.max_date,
            followers: value(s.followers),
            views_per_post: value(s.views_per_post),
            shares_per_post: value(s.shares_per_post),
            reactions_per_post: value(s.reactions_per_post),
            views_per_story: value(s.views_per_story),
            shares_per_story: value(s.shares_per_story),
            reactions_per_story: value(s.reactions_per_story),
            enabled_notifications: percent(notifications.part, notifications.total),
            growth_graph: graph(s.growth_graph),
            followers_graph: graph(s.followers_graph),
            mute_graph: graph(s.mute_graph),
            top_hours_graph: graph(s.top_hours_graph),
            interactions_graph: graph(s.interactions_graph),
            iv_interactions_graph: graph(s.iv_interactions_graph),
            views_by_source_graph: graph(s.views_by_source_graph),
            new_followers_by_source_graph: graph(s.new_followers_by_source_graph),
            languages_graph: graph(s.languages_graph),
            reactions_by_emotion_graph: graph(s.reactions_by_emotion_graph),
            story_interactions_graph: graph(s.story_interactions_graph),
            story_reactions_by_emotion_graph: graph(s.story_reactions_by_emotion_graph),
            recent_posts_interactions: s.recent_posts_interactions.into_iter().map(PostInteractions::parse).collect(),
        }
    }
}

impl PostInteractions {
    fn parse(raw: base::PostInteractionCounters) -> PostInteractions {
        use base::PostInteractionCounters as P;
        let (id, is_story, views, forwards, reactions) = match raw {
            P::PostInteractionCountersMessage(c) => (c.msg_id, false, c.views, c.forwards, c.reactions),
            P::PostInteractionCountersStory(c) => (c.story_id, true, c.views, c.forwards, c.reactions),
        };
        PostInteractions { id, is_story, views, forwards, reactions }
    }
}

fn percent(part: f64, total: f64) -> f64 {
    if total > 0.0 { part / total * 100.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::{PostInteractions, percent};
    use crate::raw::{base, types};

    #[test]
    fn percent_survives_an_empty_channel() {
        assert_eq!((percent(25.0, 100.0), percent(0.0, 0.0)), (25.0, 0.0));
    }

    #[test]
    fn counters_say_whether_they_belong_to_a_story() {
        let post = types::PostInteractionCountersMessage { msg_id: 4, views: 10, forwards: 2, reactions: 1 };
        let story = types::PostInteractionCountersStory { story_id: 9, views: 30, forwards: 0, reactions: 5 };
        use base::PostInteractionCounters as P;
        let post = PostInteractions::parse(P::PostInteractionCountersMessage(Box::new(post)));
        let story = PostInteractions::parse(P::PostInteractionCountersStory(Box::new(story)));
        assert_eq!(
            (post, story),
            (
                PostInteractions { id: 4, is_story: false, views: 10, forwards: 2, reactions: 1 },
                PostInteractions { id: 9, is_story: true, views: 30, forwards: 0, reactions: 5 },
            )
        );
    }
}
