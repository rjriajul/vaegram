use crate::Client;
use crate::dispatcher::Dispatcher;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    pub async fn run(&self) -> Result<()> {
        self.set_me(&self.get_me().await?);
        let saved = self.0.state.lock().unwrap().take();
        let mut dispatcher = match saved {
            Some(state) => Dispatcher::resume(self.clone(), state).await,
            None => {
                let base::updates::State::State(state) = self.invoke(&functions::updates::GetState {}).await?;
                Dispatcher::new(self.clone(), &state)
            }
        };
        let mut updates = self.0.updates.lock().await;
        while let Some(incoming) = updates.recv().await {
            dispatcher.handle(incoming).await;
        }
        Err(Error::Disconnected)
    }
}
