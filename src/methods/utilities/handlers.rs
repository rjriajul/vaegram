use crate::Client;
use crate::handlers::HandlerInfo;

impl Client {
    /// Lists the handlers registered now: the `on_*` routes grouped by kind, each kind in the order its handlers
    /// run, then the raw handlers and the lifecycle hooks. Next-step handlers are not listed.
    pub fn handlers(&self) -> Vec<HandlerInfo> {
        self.0.handlers.current().list()
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::filters;
    use crate::handlers::{HandlerInfo, UpdateKind};

    #[test]
    fn lists_routes_in_run_order_then_raw_handlers_and_hooks() {
        let client = Client::offline();
        let late = client.on_message(filters::all(), |_| async { Ok(()) }).group(2).stop().id();
        let first = client.on_message(filters::all(), |_| async { Ok(()) }).id();
        let gone = client.on_message(filters::all(), |_| async { Ok(()) }).id();
        let status = client.on_user_status(filters::all(), |_| async { Ok(()) }).group(-1).id();
        let raw = client.on_raw_update(|_, _| async { Ok(()) }).id();
        let start = client.on_start(|_| async { Ok(()) }).id();
        assert!(client.remove_handler(gone));
        let info = |id, kind, group, stops| HandlerInfo { id, kind, group, stops };
        assert_eq!(
            client.handlers(),
            [
                info(first, UpdateKind::Message, Some(0), false),
                info(late, UpdateKind::Message, Some(2), true),
                info(status, UpdateKind::UserStatus, Some(-1), false),
                info(raw, UpdateKind::Raw, None, false),
                info(start, UpdateKind::Start, None, false),
            ]
        );
    }
}
