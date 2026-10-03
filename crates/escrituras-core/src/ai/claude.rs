use super::stream::{role_name, OnDelta, SseParser};
use crate::state::ChatMessage;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Serialize;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Ceiling for `max_tokens`. Streaming leaves room for long answers without
/// HTTP timeouts. Models with a smaller output cap get that cap instead:
/// Anthropic returns 400 when `max_tokens` is above it.
const MAX_TOKENS: u32 = 64000;
/// Claude 3.5 Sonnet and Haiku (`claude-3-5-sonnet-20241022`,
/// `claude-3-5-haiku-20241022`)
const CLAUDE_3_5_MAX_TOKENS: u32 = 8192;
/// Claude 3 Opus, Sonnet, and Haiku (`claude-3-opus-20240229`)
const CLAUDE_3_MAX_TOKENS: u32 = 4096;
/// Beta header for `fallbacks: "default"`, which re-runs a request that
/// Claude's safety classifiers decline on a recommended fallback model
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Models that accept `fallbacks: "default"`
const FALLBACK_MODELS: &[&str] = &[
    "claude-opus-5-5",
    "claude-fable-5-1",
    "claude-opus-5",
    "claude-sonnet-5-5",
];

#[derive(Serialize)]
struct ClaudeMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<ClaudeMessage<'a>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallbacks: Option<&'static str>,
}

#[derive(Clone)]
pub struct ClaudeClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl ClaudeClient {
    pub fn new(api_key: &str) -> Self {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    /// Use a different API host (for proxies and tests)
    pub fn with_base_url(api_key: &str, base_url: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Stream a reply to `messages`, calling `on_delta` with each piece of
    /// text as it arrives. Returns the full reply.
    pub async fn stream_chat(
        &self,
        model: &str,
        system: &str,
        messages: &[ChatMessage],
        on_delta: OnDelta<'_>,
    ) -> Result<String> {
        let use_fallbacks = FALLBACK_MODELS.contains(&model);
        let request = ClaudeRequest {
            model,
            max_tokens: max_tokens_for(model),
            system,
            messages: messages
                .iter()
                .map(|m| ClaudeMessage {
                    role: role_name(m.role),
                    content: &m.content,
                })
                .collect(),
            stream: true,
            fallbacks: use_fallbacks.then_some("default"),
        };

        let mut builder = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");
        if use_fallbacks {
            builder = builder.header("anthropic-beta", FALLBACK_BETA);
        }
        let mut response = builder.json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Claude API error {}: {}", status, text));
        }

        let mut sse = SseParser::default();
        let mut reply = String::new();
        let mut stop_reason: Option<String> = None;

        let mut handle = |data: String| -> Result<()> {
            let event: serde_json::Value = serde_json::from_str(&data)?;
            match event["type"].as_str() {
                // Only text is shown; thinking and other blocks are skipped
                Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                    if let Some(text) = event["delta"]["text"].as_str() {
                        reply.push_str(text);
                        on_delta(text);
                    }
                }
                Some("message_delta") => {
                    if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                        stop_reason = Some(reason.to_string());
                    }
                }
                Some("error") => {
                    return Err(anyhow!(
                        "Claude API error: {}",
                        event["error"]["message"]
                            .as_str()
                            .unwrap_or("unknown error")
                    ));
                }
                _ => {}
            }
            Ok(())
        };

        while let Some(chunk) = response.chunk().await? {
            for data in sse.push(&chunk) {
                handle(data)?;
            }
        }
        for data in sse.finish() {
            handle(data)?;
        }

        if stop_reason.as_deref() == Some("refusal") {
            let note = "Claude declined to answer this request.";
            if reply.is_empty() {
                return Err(anyhow!(note));
            }
            let note = format!("\n\n[{}]", note);
            on_delta(&note);
            reply.push_str(&note);
        }

        Ok(reply)
    }

    pub fn list_models() -> Vec<String> {
        vec![
            "claude-opus-5-5".to_string(),
            "claude-sonnet-5-5".to_string(),
            "claude-haiku-4-5".to_string(),
            "claude-fable-5-1".to_string(),
        ]
    }
}

/// Output cap to send for `model`. Saved configs can still name ids from the
/// previous picker, which reject the streaming ceiling.
fn max_tokens_for(model: &str) -> u32 {
    if model.starts_with("claude-3-5") {
        return CLAUDE_3_5_MAX_TOKENS;
    }
    // "claude-3-<name>" is the original Claude 3 generation. "claude-3-7"
    // is Sonnet 3.7, which accepts the streaming ceiling.
    if let Some(rest) = model.strip_prefix("claude-3-") {
        if rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return CLAUDE_3_MAX_TOKENS;
        }
    }
    MAX_TOKENS
}

#[cfg(test)]
mod tests {
    use super::max_tokens_for;

    #[test]
    fn legacy_picker_models_stay_within_their_output_cap() {
        assert_eq!(max_tokens_for("claude-3-5-sonnet-20241022"), 8192);
        assert_eq!(max_tokens_for("claude-3-5-haiku-20241022"), 8192);
        assert_eq!(max_tokens_for("claude-3-opus-20240229"), 4096);
    }

    #[test]
    fn current_models_use_the_streaming_ceiling() {
        for model in [
            "claude-opus-5-5",
            "claude-sonnet-5-5",
            "claude-haiku-4-5",
            "claude-fable-5-1",
            "claude-opus-5",
            "claude-3-7-sonnet-20250219",
        ] {
            assert_eq!(max_tokens_for(model), 64000, "{model}");
        }
    }
}
