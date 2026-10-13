use super::stream::{messages_with_system, OnDelta, SseParser};
use crate::state::ChatMessage;
use anyhow::{anyhow, Result};
use reqwest::Client;

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

#[derive(Clone)]
pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAIClient {
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
        let request = serde_json::json!({
            "model": model,
            "messages": messages_with_system(system, messages),
            "stream": true,
        });

        let mut response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error {}: {}", status, text));
        }

        let mut sse = SseParser::default();
        let mut reply = String::new();

        let mut handle = |data: String| -> Result<()> {
            if data == "[DONE]" {
                return Ok(());
            }
            let event: serde_json::Value = serde_json::from_str(&data)?;
            if let Some(error) = event.get("error") {
                return Err(anyhow!(
                    "OpenAI API error: {}",
                    error["message"].as_str().unwrap_or("unknown error")
                ));
            }
            if let Some(text) = event["choices"][0]["delta"]["content"].as_str() {
                if !text.is_empty() {
                    reply.push_str(text);
                    on_delta(text);
                }
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

        Ok(reply)
    }

    pub fn list_models() -> Vec<String> {
        vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-3.5-turbo".to_string(),
        ]
    }
}
