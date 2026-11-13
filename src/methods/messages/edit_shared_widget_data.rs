use crate::{Client, errors::Result, methods::accepted, raw::functions};

/// Request built by [`Client::edit_shared_widget_data`].
#[must_use = "a request does nothing until it is awaited"]
pub struct EditSharedWidgetData<'a> {
    client: &'a Client,
    widget_id: i64,
    public_data: String,
    cache_time: Option<i32>,
}
impl Client {
    /// Replaces a shared widget's public data. An empty string clears it.
    pub fn edit_shared_widget_data(&self, widget_id: i64, public_data: impl Into<String>) -> EditSharedWidgetData<'_> {
        EditSharedWidgetData { client: self, widget_id, public_data: public_data.into(), cache_time: None }
    }
}
impl EditSharedWidgetData<'_> {
    /// Sets the public data cache time in seconds.
    pub fn cache_time(mut self, cache_time: i32) -> Self {
        self.cache_time = Some(cache_time);
        self
    }
    async fn send(self) -> Result<()> {
        accepted(
            self.client
                .invoke(&functions::messages::EditWidgetData {
                    widget_id: self.widget_id,
                    public_data: self.public_data,
                    cache_time: self.cache_time,
                })
                .await?,
        )
    }
}
into_future!(EditSharedWidgetData, ());

#[cfg(test)]
mod tests {
    use crate::{
        Client,
        raw::{Serializable, functions},
    };
    #[tokio::test]
    async fn sends_shared_widget_data_and_cache_time() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.edit_shared_widget_data(42, "data").cache_time(60).await.unwrap();
        assert_eq!(
            sent.try_recv().unwrap(),
            functions::messages::EditWidgetData { widget_id: 42, public_data: "data".into(), cache_time: Some(60) }
                .to_bytes()
        );
    }
}
