use crate::raw::base;

/// A chart from the statistics pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatisticalGraph {
    /// The chart data as JSON, as Telegram sends it. `zoom_token` loads a finer view when set.
    Data { json: String, zoom_token: Option<String> },
    /// The chart is not ready. Load it later with this token.
    Async { token: String },
    /// Telegram could not build the chart.
    Error { message: String },
}

/// A statistic with its value in the previous period.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StatisticalValue {
    /// The value in the current period.
    pub value: f64,
    /// The value in the previous period.
    pub previous_value: f64,
}

impl StatisticalGraph {
    pub(crate) fn parse(raw: base::StatsGraph) -> StatisticalGraph {
        match raw {
            base::StatsGraph::StatsGraph(g) => {
                let base::DataJSON::DataJSON(json) = g.json;
                StatisticalGraph::Data { json: json.data, zoom_token: g.zoom_token }
            }
            base::StatsGraph::StatsGraphAsync(g) => StatisticalGraph::Async { token: g.token },
            base::StatsGraph::StatsGraphError(g) => StatisticalGraph::Error { message: g.error },
        }
    }
}

impl StatisticalValue {
    pub(crate) fn parse(raw: base::StatsAbsValueAndPrev) -> StatisticalValue {
        let base::StatsAbsValueAndPrev::StatsAbsValueAndPrev(v) = raw;
        StatisticalValue { value: v.current, previous_value: v.previous }
    }
}

#[cfg(test)]
mod tests {
    use super::StatisticalGraph;
    use crate::raw::{base, types};

    #[test]
    fn graphs_keep_json_and_tokens() {
        let json = base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{\"columns\":[]}".into() }));
        let data = types::StatsGraph { json, zoom_token: Some("z".into()) };
        let cases = [
            (
                base::StatsGraph::StatsGraph(Box::new(data)),
                StatisticalGraph::Data { json: "{\"columns\":[]}".into(), zoom_token: Some("z".into()) },
            ),
            (
                base::StatsGraph::StatsGraphAsync(Box::new(types::StatsGraphAsync { token: "t".into() })),
                StatisticalGraph::Async { token: "t".into() },
            ),
            (
                base::StatsGraph::StatsGraphError(Box::new(types::StatsGraphError { error: "e".into() })),
                StatisticalGraph::Error { message: "e".into() },
            ),
        ];
        for (raw, graph) in cases {
            assert_eq!(StatisticalGraph::parse(raw), graph);
        }
    }
}
