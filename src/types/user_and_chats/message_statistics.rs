use crate::raw::base;
use crate::types::StatisticalGraph;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageStatistics {
    pub views_graph: StatisticalGraph,
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
