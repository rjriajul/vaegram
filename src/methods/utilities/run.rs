use crate::Client;
use crate::dispatcher::Dispatcher;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    pub async fn run(&self) -> Result<()> {
        self.set_me(&self.get_me().await?);
        let saved = self.0.state.lock().unwrap().take();
        let mut dispatcher = match saved {
            Some(state) => Dispatcher::resume(self.clone(), state),
            None => {
                let base::updates::State::State(state) = self.invoke(&functions::updates::GetState {}).await?;
                Dispatcher::new(self.clone(), &state)
            }
        };
        dispatcher.run(&mut *self.0.updates.lock().await).await;
        Err(Error::Disconnected)
    }
}
