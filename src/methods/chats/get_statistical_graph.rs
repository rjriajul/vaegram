use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::StatisticalGraph;

impl Client {
    pub async fn get_statistical_graph(&self, token: impl Into<String>, x: Option<i64>) -> Result<StatisticalGraph> {
        let token = token.into();
        if token.is_empty() {
            return Err(Error::InvalidArgument("get_statistical_graph needs the token of a StatisticalGraph::Async"));
        }
        let graph = self.invoke(&functions::stats::LoadAsyncGraph { token, x }).await?;
        Ok(StatisticalGraph::parse(graph))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn an_empty_token_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.get_statistical_graph("", None).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
