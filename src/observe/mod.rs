//! Watching requests: [`tagged`] marks the requests a caller causes, so an
//! [`observer`](crate::ClientBuilder::observer) can tell them apart, and ready-made observers to pass to
//! [`ClientBuilder::observer`](crate::ClientBuilder::observer).

#[cfg(feature = "metrics")]
mod metrics;

#[cfg(feature = "metrics")]
pub use self::metrics::metrics;

#[cfg(feature = "opentelemetry")]
mod opentelemetry;

#[cfg(feature = "opentelemetry")]
pub use self::opentelemetry::opentelemetry;

tokio::task_local! {
    static TAG: Option<u64>;
}

/// Runs `future` with every request it causes tagged with `tag`, which the observer sees as
/// [`Call::tag`](crate::session::Call::tag).
///
/// The tag is read on the task that sends a request, when it sends it, and travels with the request to the
/// connection's task; a request sent again after a reconnect keeps it. Requests a vaegram method sends from tasks it
/// starts for that call carry the caller's tag too: the messages a [`stream_message`](crate::Client::stream_message)
/// sends, and the `account.finishTakeoutSession` a dropped [`TakeoutSession`](crate::methods::TakeoutSession) sends.
/// So do the requests a call makes on the way, such as the peer lookups of a high-level method or the
/// authorization it exports to and imports into another data center before a download.
///
/// What vaegram does on its own stays untagged: pings, what [`run`](crate::Client::run)'s dispatcher sends (such as
/// `getDifference`) even when `run` itself is inside `tagged`, the background loop of
/// [`send_chat_action(..).repeat()`](crate::methods::SendChatAction::repeat), connect and disconnect hooks, and
/// tasks the caller spawns, since a tag stays with its task. Inside a nested `tagged`, the innermost tag wins.
pub fn tagged<F: Future>(tag: u64, future: F) -> impl Future<Output = F::Output> {
    TAG.scope(Some(tag), future)
}

/// The tag of the running task: `None` outside [`tagged`].
pub(crate) fn current_tag() -> Option<u64> {
    TAG.try_with(|tag| *tag).ok().flatten()
}

/// Runs `future` with the requests it sends untagged, for what vaegram does on its own.
pub(crate) fn untagged<F: Future>(future: F) -> impl Future<Output = F::Output> {
    TAG.scope(None, future)
}

/// `future` with the running task's tag, for a task vaegram spawns to do its caller's work.
pub(crate) fn carry_tag<F: Future>(future: F) -> impl Future<Output = F::Output> {
    TAG.scope(current_tag(), future)
}

#[cfg(test)]
mod tests {
    use super::{carry_tag, current_tag, tagged, untagged};

    #[tokio::test]
    async fn the_innermost_tag_wins_and_untagged_clears_it() {
        assert_eq!(current_tag(), None);
        let seen = tagged(1, async {
            let outer = current_tag();
            let inner = tagged(2, async { current_tag() }).await;
            let cleared = untagged(async { current_tag() }).await;
            (outer, inner, cleared, current_tag())
        })
        .await;
        assert_eq!(seen, (Some(1), Some(2), None, Some(1)));
    }

    #[tokio::test]
    async fn a_spawned_task_gets_the_tag_only_when_carried() {
        let (carried, plain) = tagged(5, async {
            let carried = tokio::spawn(carry_tag(async { current_tag() }));
            let plain = tokio::spawn(async { current_tag() });
            (carried.await.unwrap(), plain.await.unwrap())
        })
        .await;
        assert_eq!((carried, plain), (Some(5), None));
    }
}
