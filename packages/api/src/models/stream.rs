//! Streamed model replies: reads server-sent events into the same reply a
//! request without streaming gets, and keeps the text so far for the run
//! view to show while the model is still answering.

use serde_json::{json, Value};

/// Splits a byte stream into server-sent events: `(event name, data)`.
#[derive(Default)]
pub(super) struct SseParser {
    buffer: String,
}

impl SseParser {
    pub fn push(&mut self, chunk: &str) -> Vec<(Option<String>, String)> {
        self.buffer.push_str(&chunk.replace("\r\n", "\n"));
        let mut events = vec![];
        while let Some(end) = self.buffer.find("\n\n") {
            let block: String = self.buffer.drain(..end + 2).collect();
            let mut event = None;
            let mut data = vec![];
            for line in block.lines() {
                if let Some(name) = line.strip_prefix("event:") {
                    event = Some(name.trim().to_string());
                } else if let Some(value) = line.strip_prefix("data:") {
                    data.push(value.strip_prefix(' ').unwrap_or(value).to_string());
                }
            }
            if !data.is_empty() {
                events.push((event, data.join("\n")));
            }
        }
        events
    }
}

/// An error a provider sent inside the stream, if this event is one.
pub(super) fn stream_error(data: &Value) -> Option<String> {
    data.pointer("/error/message")
        .or_else(|| data.pointer("/response/error/message"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

/// A Chat Completions stream: text and tool calls arrive in pieces, and
/// usage, when asked for, in the last chunk.
#[derive(Default)]
pub(super) struct ChatStream {
    content: String,
    calls: Vec<PartialCall>,
    usage: Value,
}

impl ChatStream {
    pub fn apply(&mut self, data: &Value) {
        if data["usage"].is_object() {
            self.usage = data["usage"].clone();
        }
        let delta = &data["choices"][0]["delta"];
        if let Some(text) = delta["content"].as_str() {
            self.content.push_str(text);
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            let index = call["index"].as_u64().map_or(self.calls.len(), |i| i as usize);
            while self.calls.len() <= index {
                self.calls.push(PartialCall::default());
            }
            let slot = &mut self.calls[index];
            if let Some(id) = call["id"].as_str() {
                slot.id = id.to_string();
            }
            if let Some(name) = call["function"]["name"].as_str() {
                if slot.name.is_empty() {
                    slot.name = name.to_string();
                }
            }
            if let Some(arguments) = call["function"]["arguments"].as_str() {
                slot.arguments.push_str(arguments);
            }
        }
    }

    pub fn live(&self) -> String {
        live_text(&self.content, self.calls.iter().map(|c| c.name.as_str()))
    }

    /// The whole reply, as a request without streaming would have got it.
    pub fn finish(self) -> Value {
        let mut message = json!({
            "role": "assistant",
            "content": if self.content.is_empty() { Value::Null } else { json!(self.content) },
        });
        if !self.calls.is_empty() {
            let calls: Vec<Value> = self
                .calls
                .into_iter()
                .map(|c| json!({"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}))
                .collect();
            message["tool_calls"] = Value::Array(calls);
        }
        let mut reply = json!({ "choices": [{ "index": 0, "message": message }] });
        if self.usage.is_object() {
            reply["usage"] = self.usage;
        }
        reply
    }
}

/// A Responses-style stream: text deltas to show, and the whole response
/// in the `response.completed` event.
#[derive(Default)]
pub(super) struct ResponsesStream {
    text: String,
    calls: Vec<String>,
    completed: Option<Value>,
}

impl ResponsesStream {
    pub fn apply(&mut self, data: &Value) {
        match data["type"].as_str().unwrap_or_default() {
            "response.output_text.delta" => self.text.push_str(data["delta"].as_str().unwrap_or_default()),
            "response.output_item.added" if data["item"]["type"] == "function_call" => {
                self.calls.push(data["item"]["name"].as_str().unwrap_or_default().to_string());
            }
            "response.completed" => self.completed = Some(data["response"].clone()),
            _ => {}
        }
    }

    pub fn live(&self) -> String {
        live_text(&self.text, self.calls.iter().map(String::as_str))
    }

    /// The completed response, untranslated; `None` if it never came.
    pub fn finish(self) -> Option<Value> {
        self.completed
    }
}

/// The text so far, then a line for each tool being called.
fn live_text<'a>(text: &str, calls: impl Iterator<Item = &'a str>) -> String {
    let mut out = text.to_string();
    for name in calls.filter(|n| !n.is_empty()) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("→ {name}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_split_across_chunks_are_put_back_together() {
        let mut parser = SseParser::default();
        assert!(parser.push("data: {\"a\":").is_empty());
        let events = parser.push("1}\r\n\r\nevent: done\ndata: [DONE]\n\n");
        assert_eq!(events, [(None, "{\"a\":1}".to_string()), (Some("done".into()), "[DONE]".to_string())]);
    }

    #[test]
    fn chat_deltas_add_up_to_one_message() {
        let mut stream = ChatStream::default();
        for chunk in [
            json!({"choices": [{"delta": {"role": "assistant", "content": "Look"}}]}),
            json!({"choices": [{"delta": {"content": "ing."}}]}),
            json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "c1", "type": "function", "function": {"name": "get_quote", "arguments": "{\"tic"}}]}}]}),
            json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "ker\":\"NVDA\"}"}}]}}]}),
            json!({"choices": [], "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}}),
        ] {
            stream.apply(&chunk);
        }
        assert_eq!(stream.live(), "Looking.\n→ get_quote");
        let reply = stream.finish();
        let message = &reply["choices"][0]["message"];
        assert_eq!(message["content"], "Looking.");
        assert_eq!(message["tool_calls"][0]["id"], "c1");
        assert_eq!(message["tool_calls"][0]["function"]["arguments"], "{\"ticker\":\"NVDA\"}");
        assert_eq!(reply["usage"]["total_tokens"], 5);
    }
}
