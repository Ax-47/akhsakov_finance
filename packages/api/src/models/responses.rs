//! Translates between the chat-completions shape the trader loop works in
//! and Responses-style APIs, for providers that offer only those. Request
//! bodies go out translated; replies come back in the chat shape, so
//! nothing else needs to know which style a profile uses.

use serde_json::{json, Map, Value};

/// A chat-completions request body as a Responses request body.
pub(super) fn request(chat: &Value) -> Value {
    let mut instructions = vec![];
    let mut input = vec![];
    for message in chat["messages"].as_array().into_iter().flatten() {
        let text = message["content"].as_str().unwrap_or_default();
        match message["role"].as_str().unwrap_or_default() {
            "system" | "developer" => instructions.push(text.to_string()),
            "tool" => input.push(json!({
                "type": "function_call_output",
                "call_id": message["tool_call_id"],
                "output": text,
            })),
            "assistant" => {
                if !text.is_empty() {
                    input.push(json!({"role": "assistant", "content": text}));
                }
                for call in message["tool_calls"].as_array().into_iter().flatten() {
                    input.push(json!({
                        "type": "function_call",
                        "call_id": call["id"],
                        "name": call["function"]["name"],
                        "arguments": call["function"]["arguments"],
                    }));
                }
            }
            role => input.push(json!({"role": role, "content": text})),
        }
    }
    let mut body = Map::new();
    body.insert("model".into(), chat["model"].clone());
    if !instructions.is_empty() {
        body.insert("instructions".into(), json!(instructions.join("\n\n")));
    }
    body.insert("input".into(), Value::Array(input));
    if let Some(tools) = chat["tools"].as_array() {
        let tools: Vec<Value> = tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "name": tool["function"]["name"],
                    "description": tool["function"]["description"],
                    "parameters": tool["function"]["parameters"],
                })
            })
            .collect();
        body.insert("tools".into(), Value::Array(tools));
    }
    match &chat["tool_choice"] {
        Value::Null => {}
        Value::String(choice) => {
            body.insert("tool_choice".into(), json!(choice));
        }
        choice => {
            body.insert(
                "tool_choice".into(),
                json!({"type": "function", "name": choice["function"]["name"]}),
            );
        }
    }
    if let Some(max) = chat.get("max_tokens") {
        body.insert("max_output_tokens".into(), max.clone());
    }
    // Usage arrives with the completed response; stream_options is chat's.
    if let Some(stream) = chat.get("stream") {
        body.insert("stream".into(), stream.clone());
    }
    Value::Object(body)
}

/// A Responses reply in the chat-completions shape: one choice whose
/// message holds the text and the function calls, and usage in chat terms.
pub(super) fn reply(responses: &Value) -> Value {
    let mut text = vec![];
    let mut calls = vec![];
    for item in responses["output"].as_array().into_iter().flatten() {
        match item["type"].as_str().unwrap_or_default() {
            "message" => {
                for part in item["content"].as_array().into_iter().flatten() {
                    if let Some(t) = part["text"].as_str() {
                        text.push(t.to_string());
                    }
                }
            }
            "function_call" => calls.push(json!({
                "id": item["call_id"],
                "type": "function",
                "function": {"name": item["name"], "arguments": item["arguments"]},
            })),
            // Reasoning and anything newer carry nothing the trader uses.
            _ => {}
        }
    }
    let mut message = json!({"role": "assistant", "content": text.join("\n")});
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    if text.is_empty() {
        message["content"] = Value::Null;
    }
    let usage = &responses["usage"];
    let mut out = json!({ "choices": [{ "index": 0, "message": message }] });
    if usage.is_object() {
        out["usage"] = json!({
            "prompt_tokens": usage["input_tokens"],
            "completion_tokens": usage["output_tokens"],
            "total_tokens": usage["total_tokens"],
        });
    }
    // A reply with no output at all reads as no message, as in chat.
    if responses["output"].as_array().is_none_or(Vec::is_empty) {
        out["choices"] = json!([]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_request_becomes_a_responses_request() {
        let chat = json!({
            "model": "m",
            "messages": [
                {"role": "system", "content": "Be careful."},
                {"role": "user", "content": "Trade."},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "c1", "type": "function", "function": {"name": "get_quote", "arguments": "{\"ticker\":\"NVDA\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "c1", "content": "{\"price\":200}"}
            ],
            "tools": [{"type": "function", "function": {"name": "get_quote", "description": "d", "parameters": {"type": "object"}}}],
            "tool_choice": {"type": "function", "function": {"name": "get_quote"}},
            "max_tokens": 512
        });
        let body = request(&chat);
        assert_eq!(body["instructions"], "Be careful.");
        assert_eq!(body["input"][0], json!({"role": "user", "content": "Trade."}));
        assert_eq!(
            body["input"][1],
            json!({"type": "function_call", "call_id": "c1", "name": "get_quote", "arguments": "{\"ticker\":\"NVDA\"}"})
        );
        assert_eq!(body["input"][2], json!({"type": "function_call_output", "call_id": "c1", "output": "{\"price\":200}"}));
        assert_eq!(body["tools"][0], json!({"type": "function", "name": "get_quote", "description": "d", "parameters": {"type": "object"}}));
        assert_eq!(body["tool_choice"], json!({"type": "function", "name": "get_quote"}));
        assert_eq!(body["max_output_tokens"], 512);
        assert_eq!(request(&json!({"model": "m", "messages": [], "tool_choice": "auto"}))["tool_choice"], "auto");
    }

    #[test]
    fn a_responses_reply_reads_as_chat() {
        let reply = reply(&json!({
            "output": [
                {"type": "reasoning", "summary": []},
                {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Buying."}]},
                {"type": "function_call", "call_id": "c9", "name": "place_order", "arguments": "{}"}
            ],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        }));
        let message = &reply["choices"][0]["message"];
        assert_eq!(message["content"], "Buying.");
        assert_eq!(message["tool_calls"][0]["id"], "c9");
        assert_eq!(message["tool_calls"][0]["function"]["name"], "place_order");
        assert_eq!(reply["usage"], json!({"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}));
        let calls_only = super::reply(&json!({"output": [{"type": "function_call", "call_id": "a", "name": "x", "arguments": "{}"}]}));
        assert!(calls_only["choices"][0]["message"]["content"].is_null());
        assert!(calls_only.get("usage").is_none());
        assert_eq!(super::reply(&json!({"output": []}))["choices"], json!([]));
    }
}
