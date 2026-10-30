//! The wire layer: one `Transport` seam, an OpenAI-compatible
//! chat-completions codec, retry with backoff. A wire-format fix never
//! touches a tool bound.

/// How long one model round trip may take before the exchange is abandoned.
pub const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Largest response body the stock transport will accumulate. A response body
/// becomes a model reply that becomes a prompt, so the bound belongs to the
/// read, for the same reason the file tool's read bound does.
pub const HTTP_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Retry policy for transient model-API failures: 429 and 5xx are worth one
/// more try (2s, then 5s); anything else -- a bad key, a malformed request --
/// fails immediately because the next try would fail identically.
pub const RETRY_DELAYS: [std::time::Duration; 2] = [
    std::time::Duration::from_secs(2),
    std::time::Duration::from_secs(5),
];

/// Classifies a transport error as retryable. The stock transport embeds the
/// HTTP status in its error text (`... answered HTTP 429: ...`), so the
/// classification reads that back; a custom transport keeps the same contract
/// or opts out of retry by never naming a retryable status.
fn retryable(error: &str) -> bool {
    const MARKERS: [&str; 6] = [
        " HTTP 429",
        " HTTP 500",
        " HTTP 502",
        " HTTP 503",
        " HTTP 504",
        " HTTP 529",
    ];
    MARKERS.iter().any(|marker| error.contains(marker))
}

/// Sends one request with the retry policy above. Real sleeps only in
/// release builds; tests pass a sleeper so they stay instant.
pub fn post_with_retry<Sleep>(
    transport: &dyn Transport,
    url: &str,
    bearer: &str,
    body: &str,
    sleep: Sleep,
) -> Result<String, String>
where
    Sleep: Fn(std::time::Duration),
{
    let mut attempt = 0usize;
    loop {
        match transport.post_json(url, bearer, body, &[]) {
            Ok(body) => return Ok(body),
            Err(error) if attempt < RETRY_DELAYS.len() && retryable(&error) => {
                sleep(RETRY_DELAYS[attempt]);
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// One HTTP round trip, so the turn loop can be tested without a network.
///
/// Deliberately this small: the harness posts one JSON body and reads one
/// JSON body back. Anything richer would be a general HTTP client.
pub trait Transport {
    /// POSTs `body` as `application/json` and returns the response body.
    ///
    /// A non-2xx status is an `Err` carrying the status and as much of the
    /// body as the endpoint sent, since that is where an API states why it
    /// refused (a bad key, a rejected model name).
    fn post_json(
        &self,
        url: &str,
        bearer: &str,
        body: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<String, String>;

    /// Compatibility for caller-owned transports with their own timeout
    /// policy. The default ignores the timeout.
    fn post_json_with_timeout(
        &self,
        url: &str,
        bearer: &str,
        body: &str,
        extra_headers: &[(&str, &str)],
        _timeout: std::time::Duration,
    ) -> Result<String, String> {
        self.post_json(url, bearer, body, extra_headers)
    }
}

/// The stock transport over [`ureq`]. One bounded POST; no redirects (a
/// redirected POST replays the bearer token to a host the caller never
/// named), one bounded body.
#[derive(Clone, Copy, Debug, Default)]
pub struct HttpTransport;

impl Transport for HttpTransport {
    fn post_json(
        &self,
        url: &str,
        bearer: &str,
        body: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<String, String> {
        self.post_json_with_timeout(url, bearer, body, extra_headers, HTTP_TIMEOUT)
    }

    fn post_json_with_timeout(
        &self,
        url: &str,
        bearer: &str,
        body: &str,
        extra_headers: &[(&str, &str)],
        timeout: std::time::Duration,
    ) -> Result<String, String> {
        let timeout = timeout.min(HTTP_TIMEOUT);
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(15))
            .timeout(timeout)
            .redirects(0)
            .build();
        let mut request = agent
            .post(url)
            .set("Authorization", &format!("Bearer {bearer}"))
            .set("Content-Type", "application/json")
            .set("Accept", "application/json");
        for (name, value) in extra_headers {
            request = request.set(name, value);
        }
        let response = request.send_string(body).map_err(|error| match error {
            ureq::Error::Status(code, response) => {
                let text = response
                    .into_string()
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                format!("harness: the endpoint answered HTTP {code}: {text}")
            }
            other => format!("harness: the request failed: {other}"),
        })?;
        let mut text = String::new();
        response
            .into_reader()
            .take((HTTP_MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_string(&mut text)
            .map_err(|error| format!("harness: the response body could not be read: {error}"))?;
        if text.len() > HTTP_MAX_RESPONSE_BYTES {
            return Err(format!(
                "harness: the response body is cut off at {HTTP_MAX_RESPONSE_BYTES} bytes; a \
                 truncated body is not parsed as if it were complete"
            ));
        }
        Ok(text)
    }
}

use std::io::Read as _;

/// One tool call the model asked for, with its arguments still unparsed.
///
/// The arguments arrive as a JSON string inside a JSON object, so they are
/// carried verbatim and parsed at dispatch: a malformed argument object is
/// then one tool's bounded failure, reportable back to the model, rather than
/// a parse error that kills the whole reply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// What one model reply amounted to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelReply {
    /// A final answer: the loop stops here.
    Text(String),
    /// Work to do before the model can answer.
    ToolCalls(Vec<WireToolCall>),
}

/// The two tools, as the wire wants tool definitions written. Both are
/// advertised **always**, including when `--allow-cmd` is empty: an empty
/// allow-list makes every `exec` call refused; it does not hide the tool, so
/// a model cannot tell "not allowed yet" from "tool absent" by probing.
fn tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "file",
                "description": "Read or write one UTF-8 text file, confined to the task root. \
                                Paths are relative to that root; an absolute path, a `..` \
                                component or a symlink is refused, never rewritten.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "op": {
                            "type": "string",
                            "enum": ["read", "write", "edit"],
                            "description": "Which operation to perform."
                        },
                        "path": {
                            "type": "string",
                            "description": "Path relative to the task root."
                        },
                        "contents": {
                            "type": "string",
                            "description": "The full new contents, for op=write only."
                        },
                        "old": {
                            "type": "string",
                            "description": "Exact text to replace, for op=edit only. Must \
                                            occur exactly once: 0 or 2+ occurrences are \
                                            refused and nothing is written."
                        },
                        "new": {
                            "type": "string",
                            "description": "The replacement text, for op=edit only."
                        }
                    },
                    "required": ["op", "path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "exec",
                "description": "Run one allow-listed command in the task root as an argv \
                                vector. There is no shell: `;`, `&&`, `|` and `$(...)` are \
                                ordinary argument bytes. A command outside the allow-list is \
                                refused.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "why": {
                            "type": "string",
                            "description": "One short line saying what this command is for."
                        },
                        "argv": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "The program's executable basename followed by its \
                                            arguments."
                        }
                    },
                    "required": ["why", "argv"]
                }
            }
        }
    ])
}

/// Serializes one request body: the model, the conversation so far, and both
/// tool definitions.
pub fn request_body(model: &str, messages: &[serde_json::Value]) -> String {
    serde_json::json!({
        "model": model,
        "messages": messages,
        "tools": tool_definitions(),
        // No sampling knobs and no streaming: one bounded task wants one
        // reproducible round trip, and a streamed body would need a framing
        // parser this transport deliberately does not have.
        "stream": false,
    })
    .to_string()
}

/// Parses one response body into either a final text answer or tool calls.
///
/// A reply carrying both is treated as tool calls: the text is the model
/// narrating what it is about to do, not an answer, and stopping there would
/// drop the work it asked for.
pub fn parse_reply(body: &str) -> Result<ModelReply, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| format!("harness: the reply is not JSON: {error}"))?;
    // An API states a refusal in its own error object; surface that verbatim
    // rather than reporting a missing field.
    if let Some(message) = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
    {
        return Err(format!(
            "harness: the backend refused the request: {message}"
        ));
    }
    let message = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| "harness: the reply has no choices[0].message".to_owned())?;

    if let Some(calls) = message
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
    {
        let mut parsed = Vec::with_capacity(calls.len());
        for call in calls {
            let function = call
                .get("function")
                .ok_or_else(|| "harness: a tool call has no function object".to_owned())?;
            let name = function
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "harness: a tool call has no function name".to_owned())?;
            parsed.push(WireToolCall {
                // An id is how a tool result is matched to its call. A reply
                // without one is unanswerable, so refuse rather than invent.
                id: call
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| format!("harness: tool call {name:?} has no id"))?
                    .to_owned(),
                name: name.to_owned(),
                arguments: function
                    .get("arguments")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("{}")
                    .to_owned(),
            });
        }
        if !parsed.is_empty() {
            return Ok(ModelReply::ToolCalls(parsed));
        }
    }
    let text = message
        .get("content")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "harness: the reply has neither tool calls nor text content".to_owned())?;
    Ok(ModelReply::Text(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_carries_model_messages_and_both_tools() {
        let messages = vec![
            serde_json::json!({"role":"system","content":"s"}),
            serde_json::json!({"role":"user","content":"t"}),
        ];
        let body: serde_json::Value =
            serde_json::from_str(&request_body("fixture", &messages)).unwrap();
        assert_eq!(body["model"], "fixture");
        assert_eq!(body["tools"].as_array().map(Vec::len), Some(2));
        assert_eq!(body["stream"], false);
    }

    #[test]
    fn parse_reply_reads_text_and_error_objects() {
        let text = serde_json::json!({"choices":[{"message":{"role":"assistant","content":"hi"}}]})
            .to_string();
        assert_eq!(
            parse_reply(&text).unwrap(),
            ModelReply::Text("hi".to_owned())
        );

        let error = serde_json::json!({"error":{"message":"bad key"}}).to_string();
        assert!(parse_reply(&error).unwrap_err().contains("bad key"));

        let empty = serde_json::json!({"choices":[]}).to_string();
        assert!(parse_reply(&empty).unwrap_err().contains("no choices"));
    }

    #[test]
    fn parse_reply_prefers_tool_calls_over_narration() {
        let body = serde_json::json!({"choices":[{"message":{
            "role":"assistant",
            "content":"I will write the file now.",
            "tool_calls":[{
                "id":"c1","type":"function",
                "function":{"name":"exec","arguments":"{\"argv\":[\"ls\"]}"}
            }]
        }}]})
        .to_string();
        match parse_reply(&body).unwrap() {
            ModelReply::ToolCalls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "exec");
                assert_eq!(calls[0].id, "c1");
            }
            other => panic!("expected tool calls, got {other:?}"),
        }
    }

    fn text_reply(text: &str) -> String {
        serde_json::json!({"choices":[{"message":{"role":"assistant","content":text}}]}).to_string()
    }

    #[test]
    fn retry_5xx_then_succeed() {
        struct FlakyTransport(std::cell::Cell<usize>);
        impl Transport for FlakyTransport {
            fn post_json(
                &self,
                _u: &str,
                _b: &str,
                _body: &str,
                _h: &[(&str, &str)],
            ) -> Result<String, String> {
                let n = self.0.get();
                self.0.set(n + 1);
                if n < 2 {
                    Err("harness: the endpoint answered HTTP 503: busy".to_owned())
                } else {
                    Ok(text_reply("done"))
                }
            }
        }
        let t = FlakyTransport(std::cell::Cell::new(0));
        let slept = std::cell::RefCell::new(Vec::new());
        let reply = post_with_retry(&t, "u", "k", "{}", |d| {
            slept.borrow_mut().push(d);
        })
        .unwrap();
        assert_eq!(reply, text_reply("done"));
        assert_eq!(slept.borrow().len(), 2);
        assert_eq!(slept.borrow()[0], std::time::Duration::from_secs(2));
        assert_eq!(slept.borrow()[1], std::time::Duration::from_secs(5));
    }

    #[test]
    fn retry_exhausts_after_three_5xx() {
        struct Always503;
        impl Transport for Always503 {
            fn post_json(
                &self,
                _u: &str,
                _b: &str,
                _body: &str,
                _h: &[(&str, &str)],
            ) -> Result<String, String> {
                Err("harness: the endpoint answered HTTP 503: busy".to_owned())
            }
        }
        let error = post_with_retry(&Always503, "u", "k", "{}", |_| {}).unwrap_err();
        assert!(error.contains("503"));
    }

    #[test]
    fn retry_never_retries_401() {
        struct Always401(std::cell::Cell<usize>);
        impl Transport for Always401 {
            fn post_json(
                &self,
                _u: &str,
                _b: &str,
                _body: &str,
                _h: &[(&str, &str)],
            ) -> Result<String, String> {
                self.0.set(self.0.get() + 1);
                Err("harness: the endpoint answered HTTP 401: bad key".to_owned())
            }
        }
        let t = Always401(std::cell::Cell::new(0));
        let error = post_with_retry(&t, "u", "k", "{}", |_| {}).unwrap_err();
        assert!(error.contains("401"));
        assert_eq!(t.0.get(), 1, "401 must not retry");
    }

    #[test]
    fn parse_reply_refuses_a_tool_call_without_an_id() {
        let body = serde_json::json!({"choices":[{"message":{
            "role":"assistant", "content":null,
            "tool_calls":[{"type":"function",
                "function":{"name":"file","arguments":"{}"}}]
        }}]})
        .to_string();
        assert!(parse_reply(&body).unwrap_err().contains("no id"));
    }
}
