use crate::raw::base;
use crate::types::StatisticalGraph;

/// Statistics for one channel message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MessageStatistics {
    /// Views of the message over time.
    pub views_graph: StatisticalGraph,
    /// Reactions on the message, split by emoji.
    pub reactions_by_emotion_graph: StatisticalGraph,
}

impl MessageStatistics {
    pub(crate) fn parse(views: base::StatsGraph, reactions: base::StatsGraph) -> MessageStatistics {
        MessageStatistics {
            views_graph: StatisticalGraph::parse(views),
            reactions_by_emotion_graph: StatisticalGraph::parse(reactions),
        }
    }
}
