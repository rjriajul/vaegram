use crate::raw::base;
use crate::types::StatisticalGraph;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollStatistics {
    pub votes_graph: StatisticalGraph,
}

impl PollStatistics {
    pub(crate) fn parse(raw: base::stats::PollStats) -> PollStatistics {
        let base::stats::PollStats::PollStats(s) = raw;
        PollStatistics { votes_graph: StatisticalGraph::parse(s.votes_graph) }
    }
}
