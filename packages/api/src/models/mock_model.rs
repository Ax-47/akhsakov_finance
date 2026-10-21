//! A scripted OpenAI-compatible model endpoint for tests. Each model name has
//! a queue of replies to `POST /v1/chat/completions`, served on a random
//! local port, so whole trader runs are tested without the network.

use dioxus::server::axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

/// Tokens each successful reply reports, as `(prompt, completion)`.
pub(super) const REPLY_TOKENS: (u64, u64) = (10, 5);

/// One reply from the mock endpoint.
#[derive(Clone)]
pub(super) enum Reply {
    /// 200 with this body.
    Ok(Value),
    /// This status with this raw body.
    Raw(u16, String),
    /// Waits, then sends the inner reply.
    Delayed(Duration, Box<Reply>),
    /// The inner reply with this header added.
    Header(&'static str, String, Box<Reply>),
}

impl Reply {
    /// The assistant calls these tools, given as `(name, arguments)`.
    pub fn tools(calls: &[(&str, Value)]) -> Self {
        let calls: Vec<Value> = calls
            .iter()
            .enumerate()
            .map(|(i, (name, args))| {
                json!({
                    "id": format!("call_{i}"),
                    "type": "function",
                    "function": {"name": name, "arguments": args.to_string()}
                })
            })
            .collect();
        Self::message(json!({"role":"assistant","content":null,"tool_calls":calls}))
    }

    /// The assistant answers in text and calls no tools.
    pub fn text(content: &str) -> Self {
        Self::message(json!({"role":"assistant","content":content}))
    }

    /// A completion whose only choice is `message`.
    pub fn message(message: Value) -> Self {
        let (prompt, completion) = REPLY_TOKENS;
        Self::Ok(json!({
            "id": "chatcmpl-mock",
            "object": "chat.completion",
            "choices": [{"index": 0, "message": message}],
            "usage": {
                "prompt_tokens": prompt,
                "completion_tokens": completion,
                "total_tokens": prompt + completion
            }
        }))
    }

    /// A provider error in the OpenAI shape.
    pub fn error(status: u16, message: &str) -> Self {
        Self::Raw(status, json!({"error": {"message": message}}).to_string())
    }

    /// The same reply, sent after `delay`.
    pub fn after(self, delay: Duration) -> Self {
        Self::Delayed(delay, Box::new(self))
    }

    /// The same reply with a `name: value` header.
    pub fn header(self, name: &'static str, value: &str) -> Self {
        Self::Header(name, value.into(), Box::new(self))
    }
}

/// A request the endpoint received.
#[derive(Clone)]
pub(super) struct Received {
    pub authorization: Option<String>,
    pub body: Value,
}

#[derive(Clone, Default)]
struct Script {
    replies: Arc<Mutex<HashMap<String, VecDeque<Reply>>>>,
    received: Arc<Mutex<HashMap<String, Vec<Received>>>>,
}

pub(super) struct MockModel {
    base_url: String,
    script: Script,
}

impl MockModel {
    /// Serves the endpoint until the test's runtime ends.
    pub async fn start() -> Self {
        let script = Script::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the mock model");
        let address = listener.local_addr().expect("mock model address");
        let app = Router::new()
            .route("/v1/chat/completions", post(chat))
            .with_state(script.clone());
        tokio::spawn(async move { dioxus::server::axum::serve(listener, app).await });
        Self {
            base_url: format!("http://{address}/v1"),
            script,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Queues replies for requests naming `model`. Once they run out, the
    /// endpoint answers 500.
    pub fn script(&self, model: &str, replies: impl IntoIterator<Item = Reply>) {
        self.script
            .replies
            .lock()
            .unwrap()
            .entry(model.into())
            .or_default()
            .extend(replies);
    }

    /// The requests that named `model`, oldest first.
    pub fn received(&self, model: &str) -> Vec<Received> {
        self.script
            .received
            .lock()
            .unwrap()
            .get(model)
            .cloned()
            .unwrap_or_default()
    }
}

async fn chat(State(script): State<Script>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let model = body["model"].as_str().unwrap_or_default().to_string();
    let authorization = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    script
        .received
        .lock()
        .unwrap()
        .entry(model.clone())
        .or_default()
        .push(Received { authorization, body });
    let next = script
        .replies
        .lock()
        .unwrap()
        .get_mut(&model)
        .and_then(VecDeque::pop_front);
    let mut reply =
        next.unwrap_or_else(|| Reply::error(500, &format!("No reply scripted for {model}.")));
    let mut headers = vec![];
    let mut response = loop {
        match reply {
            Reply::Delayed(delay, inner) => {
                tokio::time::sleep(delay).await;
                reply = *inner;
            }
            Reply::Header(name, value, inner) => {
                headers.push((name, value));
                reply = *inner;
            }
            Reply::Ok(value) => break Json(value).into_response(),
            Reply::Raw(status, body) => {
                let status = StatusCode::from_u16(status).expect("a valid status");
                break (status, body).into_response();
            }
        }
    };
    for (name, value) in headers {
        response
            .headers_mut()
            .insert(name, value.parse().expect("a valid header value"));
    }
    response
}
