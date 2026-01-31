use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};


#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
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
            base_url: base_url.to_string(),
        }
    }
    
    pub async fn query(&self, model: &str, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            format: None,
        };
        
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!(
                "Ollama request failed with status: {}. Make sure Ollama is running with: ollama serve", 
                response.status()
            ));
        }
        
        let ollama_response: OllamaResponse = response.json().await?;
        Ok(ollama_response.response)
    }
    
    #[allow(dead_code)]
    pub async fn query_json(&self, model: &str, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            format: Some("json".to_string()),
        };
        
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!(
                "Ollama JSON request failed with status: {}. Make sure Ollama is running with: ollama serve", 
                response.status()
            ));
        }
        
        let ollama_response: OllamaResponse = response.json().await?;
        Ok(ollama_response.response)
    }
    
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);
        
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to list models: {}", response.status()));
        }
        
        let models_response: OllamaModelsResponse = response.json().await?;
        let model_names: Vec<String> = models_response
            .models
            .into_iter()
            .map(|model| model.name)
            .collect();
            
        Ok(model_names)
    }
    
    #[allow(dead_code)]
    pub async fn has_model(&self, name: &str) -> Result<bool> {
        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m == name))
    }
}