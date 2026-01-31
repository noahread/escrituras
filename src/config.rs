use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, anyhow};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub provider: Option<String>,
    pub default_model: Option<String>,
    pub claude_api_key: Option<String>,
    pub openai_api_key: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            provider: Some("ollama".to_string()),
            default_model: None,
            claude_api_key: None,
            openai_api_key: None,
        }
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        
        if !config_path.exists() {
            return Ok(Self::new());
        }
        
        let config_content = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&config_content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        
        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let config_content = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, config_content)?;
        Ok(())
    }

    pub fn save_default_model(model: &str) -> Result<()> {
        let mut config = Self::load().unwrap_or_else(|_| Self::new());
        config.default_model = Some(model.to_string());
        config.save()
    }

    fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("Could not determine config directory"))?;
        
        Ok(config_dir.join("escrituras").join("config.json"))
    }
}
