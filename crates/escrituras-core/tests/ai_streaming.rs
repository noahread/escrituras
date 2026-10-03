//! Streaming chat against local mock servers that mimic each provider's wire format.

use escrituras_core::{ChatMessage, ChatRole, ClaudeClient, OllamaClient, OpenAIClient};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// The request the mock server received
struct Captured {
    head: String,
    body: serde_json::Value,
}

impl Captured {
    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
    }
}

/// Serve one request, replying with `status` and writing `body_parts` as
/// separate writes so the client sees them as separate chunks.
async fn mock_server(
    status: &'static str,
    content_type: &'static str,
    body_parts: Vec<&'static str>,
) -> (String, JoinHandle<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());

    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        // Read headers, then the body by Content-Length
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        let header_end = loop {
            let n = socket.read(&mut buf).await.unwrap();
            data.extend_from_slice(&buf[..n]);
            if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
        };
        let head = String::from_utf8_lossy(&data[..header_end]).to_string();
        let captured_len = head
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.eq_ignore_ascii_case("content-length")
                    .then(|| v.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        while data.len() < header_end + captured_len {
            let n = socket.read(&mut buf).await.unwrap();
            data.extend_from_slice(&buf[..n]);
        }
        let body = serde_json::from_slice(&data[header_end..header_end + captured_len]).unwrap();

        let response_head = format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
            status, content_type
        );
        socket.write_all(response_head.as_bytes()).await.unwrap();
        for part in body_parts {
            socket.write_all(part.as_bytes()).await.unwrap();
            socket.flush().await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        socket.shutdown().await.unwrap();

        Captured { head, body }
    });

    (url, handle)
}

fn conversation() -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: ChatRole::User,
            content: "What is faith?".to_string(),
        },
        ChatMessage {
            role: ChatRole::Assistant,
            content: "See Alma 32:21.".to_string(),
        },
        ChatMessage {
            role: ChatRole::User,
            content: "And hope?".to_string(),
        },
    ]
}

// ---- Claude ---------------------------------------------------------------

const CLAUDE_STREAM: &[&str] = &[
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    // An event split across two network chunks
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"del",
    "ta\":{\"type\":\"text_delta\",\"text\":\"Hope is \"}}\n\n",
    "event: ping\ndata: {\"type\":\"ping\"}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"in Ether 12:4.\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":12}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
];

#[tokio::test]
async fn claude_streams_text_and_sends_messages() {
    let (url, server) = mock_server("200 OK", "text/event-stream", CLAUDE_STREAM.to_vec()).await;
    let client = ClaudeClient::with_base_url("test-key", &url);

    let mut deltas = Vec::new();
    let reply = client
        .stream_chat("claude-opus-5-5", "Be helpful", &conversation(), &mut |d| {
            deltas.push(d.to_string())
        })
        .await
        .unwrap();

    assert_eq!(deltas, ["Hope is ", "in Ether 12:4."]);
    assert_eq!(reply, "Hope is in Ether 12:4.");

    let request = server.await.unwrap();
    assert!(request.head.starts_with("POST /v1/messages "));
    assert_eq!(request.header("x-api-key").as_deref(), Some("test-key"));
    assert_eq!(
        request.header("anthropic-version").as_deref(),
        Some("2023-06-01")
    );
    assert_eq!(
        request.header("anthropic-beta").as_deref(),
        Some("server-side-fallback-2026-07-01")
    );
    assert_eq!(request.body["model"], "claude-opus-5-5");
    assert_eq!(request.body["system"], "Be helpful");
    assert_eq!(request.body["stream"], true);
    assert_eq!(request.body["fallbacks"], "default");
    assert_eq!(
        request.body["messages"],
        serde_json::json!([
            {"role": "user", "content": "What is faith?"},
            {"role": "assistant", "content": "See Alma 32:21."},
            {"role": "user", "content": "And hope?"}
        ])
    );
}

#[tokio::test]
async fn claude_omits_fallbacks_for_models_without_them() {
    let (url, server) = mock_server("200 OK", "text/event-stream", CLAUDE_STREAM.to_vec()).await;
    let client = ClaudeClient::with_base_url("test-key", &url);
    client
        .stream_chat("claude-haiku-4-5", "s", &conversation(), &mut |_| {})
        .await
        .unwrap();

    let request = server.await.unwrap();
    assert!(request.body.get("fallbacks").is_none());
    assert_eq!(request.header("anthropic-beta"), None);
}

#[tokio::test]
async fn claude_refusal_without_text_is_an_error() {
    let (url, _server) = mock_server(
        "200 OK",
        "text/event-stream",
        vec![
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\"}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ],
    )
    .await;
    let client = ClaudeClient::with_base_url("k", &url);
    let err = client
        .stream_chat("claude-opus-5-5", "s", &conversation(), &mut |_| {})
        .await
        .unwrap_err();
    assert!(err.to_string().contains("declined"), "{err}");
}

#[tokio::test]
async fn claude_error_event_mid_stream_is_an_error() {
    let (url, _server) = mock_server(
        "200 OK",
        "text/event-stream",
        vec![
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Par\"}}\n\n",
            "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n",
        ],
    )
    .await;
    let client = ClaudeClient::with_base_url("k", &url);
    let err = client
        .stream_chat("claude-opus-5-5", "s", &conversation(), &mut |_| {})
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Overloaded"), "{err}");
}

#[tokio::test]
async fn claude_http_error_includes_status_and_body() {
    let (url, _server) = mock_server(
        "401 Unauthorized",
        "application/json",
        vec!["{\"type\":\"error\",\"error\":{\"type\":\"authentication_error\",\"message\":\"invalid x-api-key\"}}"],
    )
    .await;
    let client = ClaudeClient::with_base_url("bad", &url);
    let err = client
        .stream_chat("claude-opus-5-5", "s", &conversation(), &mut |_| {})
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("401"), "{err}");
    assert!(err.contains("invalid x-api-key"), "{err}");
}

// ---- OpenAI ---------------------------------------------------------------

#[tokio::test]
async fn openai_streams_text_with_system_message_first() {
    let (url, server) = mock_server(
        "200 OK",
        "text/event-stream",
        vec![
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hope \"}}]}\n\ndata: {\"choi",
            "ces\":[{\"index\":0,\"delta\":{\"content\":\"endures.\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        ],
    )
    .await;
    let client = OpenAIClient::with_base_url("sk-test", &url);

    let mut deltas = Vec::new();
    let reply = client
        .stream_chat("gpt-4o", "Be helpful", &conversation(), &mut |d| {
            deltas.push(d.to_string())
        })
        .await
        .unwrap();

    assert_eq!(deltas, ["Hope ", "endures."]);
    assert_eq!(reply, "Hope endures.");

    let request = server.await.unwrap();
    assert!(request.head.starts_with("POST /v1/chat/completions "));
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer sk-test")
    );
    assert_eq!(request.body["stream"], true);
    assert_eq!(
        request.body["messages"][0],
        serde_json::json!({"role": "system", "content": "Be helpful"})
    );
    assert_eq!(request.body["messages"][3]["content"], "And hope?");
}

// ---- Ollama ---------------------------------------------------------------

#[tokio::test]
async fn ollama_streams_ndjson_lines() {
    let (url, server) = mock_server(
        "200 OK",
        "application/x-ndjson",
        vec![
            "{\"model\":\"gemma3\",\"message\":{\"role\":\"assistant\",\"content\":\"Faith \"},\"done\":false}\n{\"model\":\"gemma3\",\"message\":{\"role\":\"assistant\",\"con",
            "tent\":\"and hope.\"},\"done\":false}\n",
            "{\"model\":\"gemma3\",\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true}\n",
        ],
    )
    .await;
    let client = OllamaClient::new(&url);

    let mut deltas = Vec::new();
    let reply = client
        .stream_chat("gemma3", "Be helpful", &conversation(), &mut |d| {
            deltas.push(d.to_string())
        })
        .await
        .unwrap();

    assert_eq!(deltas, ["Faith ", "and hope."]);
    assert_eq!(reply, "Faith and hope.");

    let request = server.await.unwrap();
    assert!(request.head.starts_with("POST /api/chat "));
    assert_eq!(request.body["model"], "gemma3");
    assert_eq!(request.body["stream"], true);
    assert_eq!(request.body["messages"][0]["role"], "system");
    assert_eq!(request.body["messages"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn ollama_error_line_is_an_error() {
    let (url, _server) = mock_server(
        "200 OK",
        "application/x-ndjson",
        vec!["{\"error\":\"model 'nope' not found\"}\n"],
    )
    .await;
    let client = OllamaClient::new(&url);
    let err = client
        .stream_chat("nope", "s", &conversation(), &mut |_| {})
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");
}
