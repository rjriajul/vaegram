use ::metrics::{SharedString, counter, histogram};

use crate::errors::Error;
use crate::session::Call;

/// Records every request in the installed `metrics` recorder, as
/// `vaegram_requests_total{method, dc, outcome}`, `vaegram_request_duration_seconds{method}` and
/// `vaegram_resends_total{method}`. `outcome` is `ok`, the RPC error's name (`FLOOD_WAIT`, without the seconds),
/// `timeout`, `disconnected` or `other`. Use it as `.observer(vaegram::observe::metrics)`.
pub fn metrics(call: &Call<'_>) {
    let outcome: SharedString = match call.result {
        Ok(_) => "ok".into(),
        // Label values must be 'static or owned and the name lives only as long as the call: failures copy it.
        Err(Error::Rpc(e)) => e.name.clone().into(),
        Err(Error::Timeout) => "timeout".into(),
        Err(Error::Disconnected) => "disconnected".into(),
        Err(_) => "other".into(),
    };
    let dc: SharedString = match call.dc {
        // Test servers reuse ids 1 to 3, so these cover nearly every call without an allocation.
        1 => "1".into(),
        2 => "2".into(),
        3 => "3".into(),
        4 => "4".into(),
        5 => "5".into(),
        other => other.to_string().into(),
    };
    counter!("vaegram_requests_total", "method" => call.name, "dc" => dc, "outcome" => outcome).increment(1);
    histogram!("vaegram_request_duration_seconds", "method" => call.name).record(call.duration.as_secs_f64());
    if call.resent > 0 {
        counter!("vaegram_resends_total", "method" => call.name).increment(u64::from(call.resent));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    use super::metrics;
    use crate::errors::{Error, RpcError};
    use crate::session::Call;

    fn call<'a>(name: &'static str, result: Result<&'a [u8], &'a Error>, resent: u16) -> Call<'a> {
        Call {
            name,
            msg_id: 1,
            request: b"request",
            duration: Duration::from_millis(500),
            result,
            dc: 2,
            connection: 0,
            resent,
        }
    }

    #[test]
    fn counts_calls_by_method_and_outcome() {
        let (flood, timeout, disconnected) =
            (Error::Rpc(RpcError::new(420, "FLOOD_WAIT_3")), Error::Timeout, Error::Disconnected);
        let other = Error::Transport(-404);
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        ::metrics::with_local_recorder(&recorder, || {
            metrics(&call("users.getUsers", Ok(b"answer"), 0));
            metrics(&call("users.getUsers", Ok(b"answer"), 0));
            metrics(&call("users.getUsers", Err(&flood), 2));
            metrics(&call("help.getConfig", Err(&timeout), 0));
            metrics(&call("help.getConfig", Err(&disconnected), 1));
            metrics(&call("help.getConfig", Err(&other), 0));
            metrics(&Call { dc: 5, ..call("upload.getCdnFile", Ok(b"x"), 0) });
            metrics(&Call { dc: 203, ..call("upload.getCdnFile", Ok(b"x"), 0) });
        });

        let mut seen: Vec<_> = snapshotter
            .snapshot()
            .into_vec()
            .into_iter()
            .map(|(key, _, _, value)| {
                let key = key.key();
                let labels: Vec<String> = key.labels().map(|l| format!("{}={}", l.key(), l.value())).collect();
                let value = match value {
                    DebugValue::Counter(n) => format!("{n}"),
                    DebugValue::Histogram(values) => format!("{:?}", values.iter().map(|v| v.0).collect::<Vec<_>>()),
                    DebugValue::Gauge(_) => "gauge".into(),
                };
                (key.name().to_owned(), labels.join(","), value)
            })
            .collect();
        seen.sort();
        let expected = [
            ("vaegram_request_duration_seconds", "method=help.getConfig", "[0.5, 0.5, 0.5]"),
            ("vaegram_request_duration_seconds", "method=upload.getCdnFile", "[0.5, 0.5]"),
            ("vaegram_request_duration_seconds", "method=users.getUsers", "[0.5, 0.5, 0.5]"),
            ("vaegram_requests_total", "method=help.getConfig,dc=2,outcome=disconnected", "1"),
            ("vaegram_requests_total", "method=help.getConfig,dc=2,outcome=other", "1"),
            ("vaegram_requests_total", "method=help.getConfig,dc=2,outcome=timeout", "1"),
            ("vaegram_requests_total", "method=upload.getCdnFile,dc=203,outcome=ok", "1"),
            ("vaegram_requests_total", "method=upload.getCdnFile,dc=5,outcome=ok", "1"),
            ("vaegram_requests_total", "method=users.getUsers,dc=2,outcome=FLOOD_WAIT", "1"),
            ("vaegram_requests_total", "method=users.getUsers,dc=2,outcome=ok", "2"),
            ("vaegram_resends_total", "method=help.getConfig", "1"),
            ("vaegram_resends_total", "method=users.getUsers", "2"),
        ];
        let expected: Vec<_> = expected.iter().map(|(n, l, v)| (n.to_string(), l.to_string(), v.to_string())).collect();
        assert_eq!(seen, expected);
    }
}
