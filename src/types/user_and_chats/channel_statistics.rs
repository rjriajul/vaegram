use crate::raw::base;
use crate::types::{StatisticalGraph, StatisticalValue};

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelStatistics {
    pub min_date: i32,
    pub max_date: i32,
    pub followers: StatisticalValue,
    pub views_per_post: StatisticalValue,
    pub shares_per_post: StatisticalValue,
    pub reactions_per_post: StatisticalValue,
    pub views_per_story: StatisticalValue,
    pub shares_per_story: StatisticalValue,
    pub reactions_per_story: StatisticalValue,
    pub enabled_notifications: f64,
    pub growth_graph: StatisticalGraph,
    pub followers_graph: StatisticalGraph,
    pub mute_graph: StatisticalGraph,
    pub top_hours_graph: StatisticalGraph,
    pub interactions_graph: StatisticalGraph,
    pub iv_interactions_graph: StatisticalGraph,
    pub views_by_source_graph: StatisticalGraph,
    pub new_followers_by_source_graph: StatisticalGraph,
    pub languages_graph: StatisticalGraph,
    pub reactions_by_emotion_graph: StatisticalGraph,
    pub story_interactions_graph: StatisticalGraph,
    pub story_reactions_by_emotion_graph: StatisticalGraph,
    pub recent_posts_interactions: Vec<PostInteractions>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostInteractions {
    pub id: i32,
    pub is_story: bool,
    pub views: i32,
    pub forwards: i32,
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
