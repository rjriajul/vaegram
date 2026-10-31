use crate::Client;
use crate::handlers::HandlerId;

impl Client {
    /// Unregisters a handler from the next update on; runs already started finish. `false` when nothing matched:
    /// it was removed before, or, for a lifecycle hook, replaced.
    pub fn remove_handler(&self, handler: HandlerId) -> bool {
        self.0.handlers.add(|r| r.remove(handler.kind, handler.id))
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::session::{Incoming, UpdatesRx};
    use crate::storage::state::UpdateState;
    use crate::types::RawUpdate;

    #[tokio::test]
    async fn a_removed_handler_gets_no_more_updates() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;

        let (tx, mut seen) = mpsc::unbounded_channel();
        let handler = |name: &'static str| {
            let tx = tx.clone();
            move |_: Client, _: RawUpdate| {
                let _ = tx.send(name);
                async { Ok(()) }
            }
        };
        let a = client.on_raw_update(handler("a")).id();
        client.on_raw_update(handler("b"));
        assert!(client.remove_handler(a));

        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        let status = types::UpdateUserStatus { user_id: 7, status: base::UserStatus::UserStatusEmpty };
        let update = types::UpdateShort { update: base::Update::UpdateUserStatus(Box::new(status)), date: 2 };
        updates
            .send(Incoming { updates: base::Updates::UpdateShort(Box::new(update)), from_request: false })
            .await
            .unwrap();
        assert_eq!(seen.recv().await, Some("b"));
        client.stop();
        assert!(run.await.unwrap().is_ok());
        assert!(seen.try_recv().is_err(), "the removed handler ran");
    }

    #[test]
    fn removing_twice_returns_false() {
        let client = Client::offline();
        let id = client.on_raw_update(|_, _| async { Ok(()) }).id();
        assert!(client.remove_handler(id));
        assert!(!client.remove_handler(id));
    }

    #[test]
    fn raw_handlers_can_be_removed() {
        let client = Client::offline();
        let id = client.on_raw_update(|_, _| async { Ok(()) }).id();
        let generation = client.0.handlers.generation();
        assert!(client.remove_handler(id));
        assert!(client.0.handlers.current().raw.is_empty());
        assert!(client.0.handlers.generation() > generation, "a running dispatcher would keep the old router");
    }
}
