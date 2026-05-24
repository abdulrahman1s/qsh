use super::{PreparedRequest, StreamKind, extract_delta};
use crate::config::Mode;
use crate::util::settings::Settings;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::watch;

#[derive(Debug)]
pub struct StreamResult {
    pub text: String,
    pub raw: String,
    pub net_err: Option<String>,
    #[allow(dead_code)]
    pub status: Option<u16>,
}

pub struct StreamHandle {
    pub join: tokio::task::JoinHandle<StreamResult>,
    pub buf: Arc<Mutex<String>>,
}

#[derive(Debug, Clone, Copy)]
pub enum FailureKind {
    Network,
    Api,
    Parse,
}

pub fn classify_failure(raw: &str, net_err: Option<&str>) -> (FailureKind, String) {
    if let Some(err) = json_error_message(raw) {
        return (FailureKind::Api, err);
    }
    for line in raw.lines() {
        if let Some(payload) = line.strip_prefix("data: ") {
            if payload == "[DONE]" {
                continue;
            }
            if let Some(err) = json_error_message(payload) {
                return (FailureKind::Api, err);
            }
        }
    }
    if let Some(err) = net_err.filter(|s| !s.is_empty()) {
        let mut joined = String::new();
        for (i, l) in err.lines().take(4).enumerate() {
            if i > 0 {
                joined.push_str("; ");
            }
            joined.push_str(l);
        }
        if joined.is_empty() {
            joined = "curl failed".into();
        }
        return (FailureKind::Network, joined);
    }
    if !raw.is_empty() {
        (
            FailureKind::Parse,
            "no command returned (raw response captured; rerun with --debug)".into(),
        )
    } else {
        (
            FailureKind::Parse,
            "no command returned (empty response)".into(),
        )
    }
}

fn json_error_message(s: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(s.trim()).ok()?;
    if let Some(err) = v.get("error") {
        match err {
            serde_json::Value::Object(_) => {
                let mut parts = Vec::new();
                if let Some(t) = err.get("type").and_then(|x| x.as_str())
                    && !t.is_empty()
                {
                    parts.push(t.to_string());
                }
                if let Some(c) = err.get("code") {
                    let cs = match c {
                        serde_json::Value::String(s) => s.clone(),
                        _ => c.to_string(),
                    };
                    if !cs.is_empty() && cs != "null" {
                        parts.push(cs);
                    }
                }
                if let Some(m) = err.get("message").and_then(|x| x.as_str())
                    && !m.is_empty()
                {
                    parts.push(m.to_string());
                }
                if !parts.is_empty() {
                    return Some(parts.join(": "));
                }
            }
            serde_json::Value::String(s) => return Some(s.clone()),
            _ => {}
        }
    }
    if let Some(m) = v.get("message").and_then(|x| x.as_str()) {
        return Some(m.to_string());
    }
    None
}

pub fn start(
    req: PreparedRequest,
    mode: Mode,
    cancel_rx: watch::Receiver<bool>,
    settings: &Settings,
) -> StreamHandle {
    let buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let buf_clone = Arc::clone(&buf);
    let timeouts = StreamTimeouts {
        connect: settings.timeout_connect(),
        total: if mode == Mode::Smart {
            settings.timeout_smart()
        } else {
            settings.timeout_fast()
        },
    };
    let join = tokio::spawn(async move { run(req, timeouts, buf_clone, cancel_rx).await });
    StreamHandle { join, buf }
}

#[derive(Debug, Clone, Copy)]
struct StreamTimeouts {
    connect: u64,
    total: u64,
}

async fn run(
    req: PreparedRequest,
    timeouts: StreamTimeouts,
    buf: Arc<Mutex<String>>,
    mut cancel_rx: watch::Receiver<bool>,
) -> StreamResult {
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(timeouts.connect))
        .timeout(Duration::from_secs(timeouts.total))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return StreamResult {
                text: String::new(),
                raw: String::new(),
                net_err: Some(format!("client build failed: {e}")),
                status: None,
            };
        }
    };

    let mut builder = client.post(&req.url);
    for (k, v) in &req.headers {
        builder = builder.header(k, v);
    }
    builder = builder.json(&req.body);

    let resp = tokio::select! {
        r = builder.send() => r,
        _ = cancel_rx.changed() => {
            return StreamResult { text: String::new(), raw: String::new(), net_err: Some("cancelled".into()), status: None };
        }
    };

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            return StreamResult {
                text: String::new(),
                raw: String::new(),
                net_err: Some(e.to_string()),
                status: None,
            };
        }
    };

    let status = resp.status();
    let kind = super::stream_filter_kind(req.provider);
    let mut raw = String::new();
    let mut stream = resp.bytes_stream();
    let mut leftover = Vec::<u8>::new();

    while let Some(chunk) = tokio::select! {
        c = stream.next() => c,
        _ = cancel_rx.changed() => None,
    } {
        match chunk {
            Ok(bytes) => {
                leftover.extend_from_slice(&bytes);
                while let Some(pos) = leftover.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = leftover.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line);
                    let trimmed = line.trim_end_matches(['\r', '\n']);
                    raw.push_str(trimmed);
                    raw.push('\n');
                    process_line(trimmed, kind, &buf).await;
                }
            }
            Err(e) => {
                return StreamResult {
                    text: buf.lock().await.clone(),
                    raw,
                    net_err: Some(e.to_string()),
                    status: Some(status.as_u16()),
                };
            }
        }
    }
    if !leftover.is_empty() {
        let line = String::from_utf8_lossy(&leftover);
        let trimmed = line.trim_end_matches(['\r', '\n']);
        raw.push_str(trimmed);
        raw.push('\n');
        process_line(trimmed, kind, &buf).await;
    }

    let cancelled = *cancel_rx.borrow();
    let net_err = if cancelled {
        Some("cancelled".into())
    } else {
        None
    };
    let text = buf.lock().await.clone();
    let net_err = if !status.is_success() && net_err.is_none() {
        Some(format!("HTTP {}", status.as_u16()))
    } else {
        net_err
    };
    StreamResult {
        text,
        raw,
        net_err,
        status: Some(status.as_u16()),
    }
}

async fn process_line(line: &str, kind: StreamKind, buf: &Arc<Mutex<String>>) {
    let Some(payload) = line.strip_prefix("data: ") else {
        return;
    };
    if payload == "[DONE]" {
        return;
    }
    if let Some(delta) = extract_delta(kind, payload) {
        buf.lock().await.push_str(&delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_api_error_with_body() {
        let raw = r#"{"error":{"type":"invalid_request_error","code":"x","message":"oops"}}"#;
        let (kind, msg) = classify_failure(raw, None);
        assert!(matches!(kind, FailureKind::Api));
        assert!(msg.contains("invalid_request_error"));
        assert!(msg.contains("oops"));
    }

    #[test]
    fn classify_api_error_in_sse() {
        let raw = "data: {\"error\":{\"message\":\"bad\"}}\n";
        let (kind, msg) = classify_failure(raw, None);
        assert!(matches!(kind, FailureKind::Api));
        assert!(msg.contains("bad"));
    }

    #[test]
    fn classify_network_when_only_stderr() {
        let (kind, msg) = classify_failure("", Some("Could not resolve host"));
        assert!(matches!(kind, FailureKind::Network));
        assert!(msg.contains("resolve"));
    }

    #[test]
    fn classify_parse_when_empty() {
        let (kind, _) = classify_failure("", None);
        assert!(matches!(kind, FailureKind::Parse));
    }
}
