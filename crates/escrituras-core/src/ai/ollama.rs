use super::stream::{messages_with_system, LineBuffer, OnDelta};
use crate::state::ChatMessage;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
    base_url: String,
}

impl OllamaClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
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
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Ollama request failed with status: {} {}. Make sure Ollama is running with: ollama serve",
                status,
                text
            ));
        }

        // One JSON object per line
        let mut lines = LineBuffer::default();
        let mut reply = String::new();

        let mut handle = |line: String| -> Result<()> {
            if line.trim().is_empty() {
                return Ok(());
            }
            let event: serde_json::Value = serde_json::from_str(&line)?;
            if let Some(error) = event["error"].as_str() {
                return Err(anyhow!("Ollama error: {}", error));
            }
            if let Some(text) = event["message"]["content"].as_str() {
                if !text.is_empty() {
                    reply.push_str(text);
                    on_delta(text);
                }
            }
            Ok(())
        };

        while let Some(chunk) = response.chunk().await? {
            for line in lines.push(&chunk) {
                handle(line)?;
            }
        }
        if let Some(line) = lines.finish() {
            handle(line)?;
        }

        Ok(reply)
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to list models: {}", response.status()));
        }

        let models_response: OllamaModelsResponse = response.json().await?;
        Ok(models_response
            .models
            .into_iter()
            .map(|model| model.name)
            .collect())
    }
}
