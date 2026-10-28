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

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::{updates, users};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::storage::state::UpdateState;

    #[tokio::test]
    async fn restart_catches_up_from_saved_state() {
        let me = vec![base::User::UserEmpty(Box::new(types::UserEmpty { id: 1 }))];
        let empty = types::updates::DifferenceEmpty { date: 2, seq: 3 };
        let empty = base::updates::Difference::DifferenceEmpty(Box::new(empty));
        let (client, mut sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });

        assert!(client.run().await.is_err());

        let id = |body: &[u8]| u32::from_le_bytes(body[..4].try_into().unwrap());
        assert_eq!(id(&sent.try_recv().unwrap()), users::GetUsers::CONSTRUCTOR_ID);
        let difference = sent.try_recv().unwrap();
        assert_eq!(id(&difference), updates::GetDifference::CONSTRUCTOR_ID);
        assert_eq!(difference[8..12], 10i32.to_le_bytes());
        assert!(sent.try_recv().is_err(), "a saved state must not ask for updates.getState");
    }
}
