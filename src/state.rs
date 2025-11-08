use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingState {
    pub symbol: String,
    pub last_price: f64,
    pub last_update_time: i64,
}

impl TradingState {
    pub fn load(path: &str) -> Result<Option<Self>> {
        if !Path::new(path).exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)
            .context(format!("Failed to read state file: {}", path))?;

        let state: TradingState = serde_json::from_str(&content)
            .context("Failed to parse state JSON")?;

        Ok(Some(state))
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn new(symbol: String, price: f64) -> Self {
        Self {
            symbol,
            last_price: price,
            last_update_time: chrono::Utc::now().timestamp(),
        }
    }
}
