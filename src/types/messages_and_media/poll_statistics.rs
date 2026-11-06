use crate::raw::base;
use crate::types::StatisticalGraph;

/// Statistics of a poll.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PollStatistics {
    /// Graph of the poll's votes.
    pub votes_graph: StatisticalGraph,
}

impl PollStatistics {
    pub(crate) fn parse(raw: base::stats::PollStats) -> PollStatistics {
        let base::stats::PollStats::PollStats(s) = raw;
        PollStatistics { votes_graph: StatisticalGraph::parse(s.votes_graph) }
    }
}
