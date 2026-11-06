use std::time::SystemTime;

use ::opentelemetry::trace::{Span, SpanBuilder, SpanKind, Status, Tracer};
use ::opentelemetry::{Context, KeyValue};

use crate::session::Call;

/// Turns every request into a finished client span named after the method, with `rpc.system`, `rpc.method`,
/// `telegram.dc`, `telegram.connection`, `telegram.msg_id` and `telegram.resent`; a failed request gets an error
/// status. The request bytes are never recorded: they can hold credentials. The observer runs on the connection's
/// task, away from the caller, so the spans are roots. Use it as `.observer(vaegram::observe::opentelemetry(tracer))`.
///
/// Give the tracer's provider a batch span processor (`with_batch_exporter`): a simple one exports each span on the
/// connection's task as it ends, so a slow exporter would stall every request on that connection.
pub fn opentelemetry<T>(tracer: T) -> impl Fn(&Call<'_>) + Send + Sync + 'static
where
    T: Tracer + Send + Sync + 'static,
{
    move |call: &Call<'_>| {
        let end = SystemTime::now();
        let start = end.checked_sub(call.duration).unwrap_or(end);
        let attributes = [
            KeyValue::new("rpc.system", "telegram"),
            KeyValue::new("rpc.method", call.name),
            KeyValue::new("telegram.dc", i64::from(call.dc)),
            KeyValue::new("telegram.connection", i64::from(call.connection)),
            KeyValue::new("telegram.msg_id", call.msg_id),
            KeyValue::new("telegram.resent", i64::from(call.resent)),
        ];
        let mut span = SpanBuilder::from_name(call.name)
            .with_kind(SpanKind::Client)
            .with_start_time(start)
            .with_attributes(attributes)
            .start_with_context(&tracer, &Context::new());
        if let Err(error) = call.result {
            span.set_status(Status::error(error.to_string()));
        }
        span.end_with_timestamp(end);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ::opentelemetry::trace::{SpanId, SpanKind, Status, TracerProvider};
    use ::opentelemetry::{KeyValue, Value};
    use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};

    use super::opentelemetry;
    use crate::errors::{Error, RpcError};
    use crate::session::Call;

    fn observed(call: &Call<'_>) -> SpanData {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder().with_simple_exporter(exporter.clone()).build();
        opentelemetry(provider.tracer("vaegram"))(call);
        let mut spans = exporter.get_finished_spans().unwrap();
        assert_eq!(spans.len(), 1);
        spans.remove(0)
    }

    fn call<'a>(request: &'a [u8], result: Result<&'a [u8], &'a Error>) -> Call<'a> {
        Call {
            name: "users.getUsers",
            msg_id: 7_000_000_000_001,
            request,
            duration: Duration::from_millis(250),
            result,
            dc: 2,
            connection: 1,
            resent: 3,
        }
    }

    #[test]
    fn a_call_becomes_a_finished_client_span() {
        let span = observed(&call(b"request", Ok(b"answer")));
        assert_eq!((&*span.name, &span.span_kind, &span.status), ("users.getUsers", &SpanKind::Client, &Status::Unset));
        assert_eq!(span.end_time.duration_since(span.start_time).ok(), Some(Duration::from_millis(250)));
        assert_eq!(span.parent_span_id, SpanId::INVALID);
        let expected = [
            KeyValue::new("rpc.system", "telegram"),
            KeyValue::new("rpc.method", "users.getUsers"),
            KeyValue::new("telegram.dc", 2_i64),
            KeyValue::new("telegram.connection", 1_i64),
            KeyValue::new("telegram.msg_id", 7_000_000_000_001_i64),
            KeyValue::new("telegram.resent", 3_i64),
        ];
        assert_eq!(span.attributes, expected);
    }

    #[test]
    fn a_failed_call_never_records_request_bytes() {
        let refused = Error::Rpc(RpcError::parse(401, "AUTH_KEY_UNREGISTERED"));
        let span = observed(&call(b"bot-token-1234:secret", Err(&refused)));
        assert!(matches!(&span.status, Status::Error { description } if description.contains("AUTH_KEY_UNREGISTERED")));
        let leaks = |value: &Value| value.as_str().contains("secret");
        assert!(!span.attributes.iter().any(|kv| leaks(&kv.value)), "{:?}", span.attributes);
        assert!(span.events.is_empty() && span.links.is_empty());
        assert_eq!(span.attributes.len(), 6);
    }
}
