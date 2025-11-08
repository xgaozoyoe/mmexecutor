use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridConfig {
    pub symbol: String,
    pub first_buy_offset_percentage: f64,   // 第一个买单距离当前价格的百分比
    pub first_sell_offset_percentage: f64,  // 第一个卖单距离当前价格的百分比
    pub buy_price_percentage: f64,
    pub sell_price_percentage: f64,
    pub grid_interval_percentage: f64,
    pub total_buy_value: f64,
    pub total_sell_value: f64,
    pub grid_levels: usize,
    pub minimal_order_value: f64,           // 最小订单总价值，低于此值不布单
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub exchange: String,  // "mexc", "gate", "kucoin"
    pub api_key: String,
    pub api_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_passphrase: Option<String>,  // KuCoin 需要
    pub grid: GridConfig,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;

        let config: Config = serde_json::from_str(&content)
            .context("Failed to parse config JSON")?;

        Self::validate(&config)?;
        Ok(config)
    }

    fn validate(config: &Config) -> Result<()> {
        if config.api_key.is_empty() {
            anyhow::bail!("API key cannot be empty");
        }
        if config.api_secret.is_empty() {
            anyhow::bail!("API secret cannot be empty");
        }
        if config.grid.symbol.is_empty() {
            anyhow::bail!("Symbol cannot be empty");
        }
        if config.grid.first_buy_offset_percentage <= 0.0 {
            anyhow::bail!("First buy offset percentage must be positive");
        }
        if config.grid.first_sell_offset_percentage <= 0.0 {
            anyhow::bail!("First sell offset percentage must be positive");
        }
        if config.grid.buy_price_percentage <= 0.0 || config.grid.buy_price_percentage >= 100.0 {
            anyhow::bail!("Buy price percentage must be between 0 and 100");
        }
        if config.grid.sell_price_percentage <= 0.0 || config.grid.sell_price_percentage >= 100.0 {
            anyhow::bail!("Sell price percentage must be between 0 and 100");
        }
        if config.grid.grid_interval_percentage <= 0.0 {
            anyhow::bail!("Grid interval percentage must be positive");
        }
        if config.grid.total_buy_value <= 0.0 {
            anyhow::bail!("Total buy value must be positive");
        }
        if config.grid.total_sell_value <= 0.0 {
            anyhow::bail!("Total sell value must be positive");
        }
        if config.grid.grid_levels == 0 {
            anyhow::bail!("Grid levels must be at least 1");
        }
        if config.grid.minimal_order_value <= 0.0 {
            anyhow::bail!("Minimal order value must be positive");
        }
        Ok(())
    }

    pub fn create_example(path: &str) -> Result<()> {
        let example = Config {
            exchange: "mexc".to_string(),  // 可选: "mexc", "gate", "kucoin"
            api_key: "your_api_key".to_string(),
            api_secret: "your_api_secret".to_string(),
            api_passphrase: None,  // KuCoin 需要
            grid: GridConfig {
                symbol: "BTCUSDT".to_string(),
                first_buy_offset_percentage: 0.5,  // 第一个买单距离当前价格 0.5%
                first_sell_offset_percentage: 0.5, // 第一个卖单距离当前价格 0.5%
                buy_price_percentage: 5.0,
                sell_price_percentage: 5.0,
                grid_interval_percentage: 0.5,
                total_buy_value: 500.0,  // 最小建议值，确保每个订单 >= 1 USDT
                total_sell_value: 500.0,
                grid_levels: 10,
                minimal_order_value: 10.0,  // 调整后的总价值低于此值不布单
            },
        };

        let json = serde_json::to_string_pretty(&example)?;
        fs::write(path, json)?;
        Ok(())
    }
}
