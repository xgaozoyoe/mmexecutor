use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidPriceMethod {
    Simple,              // 最优买卖价的简单平均
    WeightedByVolume,    // 按订单量加权
    VolumeThreshold,     // 找到累计量达到阈值的价格
}

impl Default for MidPriceMethod {
    fn default() -> Self {
        MidPriceMethod::Simple
    }
}

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

    // 中间价计算相关配置
    #[serde(default)]
    pub mid_price_method_bid: MidPriceMethod,   // Bid 价格计算方法
    #[serde(default)]
    pub mid_price_method_ask: MidPriceMethod,   // Ask 价格计算方法
    #[serde(default = "default_orderbook_depth")]
    pub orderbook_depth: u32,                   // 获取的订单簿深度
    #[serde(default = "default_volume_threshold")]
    pub volume_threshold_usdt: f64,             // VolumeThreshold 方法的默认阈值（USDT）
}

fn default_orderbook_depth() -> u32 {
    20
}

fn default_volume_threshold() -> f64 {
    100.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub name: String,  // "mexc", "gate", "kucoin"
    pub api_key: String,
    pub api_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_passphrase: Option<String>,  // KuCoin 需要
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub exchanges: Vec<ExchangeConfig>,
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
        if config.exchanges.is_empty() {
            anyhow::bail!("At least one exchange must be configured");
        }
        for (i, exchange) in config.exchanges.iter().enumerate() {
            if exchange.name.is_empty() {
                anyhow::bail!("Exchange #{} name cannot be empty", i + 1);
            }
            if exchange.api_key.is_empty() {
                anyhow::bail!("Exchange #{} ({}) API key cannot be empty", i + 1, exchange.name);
            }
            if exchange.api_secret.is_empty() {
                anyhow::bail!("Exchange #{} ({}) API secret cannot be empty", i + 1, exchange.name);
            }
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
            exchanges: vec![
                ExchangeConfig {
                    name: "mexc".to_string(),
                    api_key: "your_mexc_api_key".to_string(),
                    api_secret: "your_mexc_api_secret".to_string(),
                    api_passphrase: None,
                },
                ExchangeConfig {
                    name: "gate".to_string(),
                    api_key: "your_gate_api_key".to_string(),
                    api_secret: "your_gate_api_secret".to_string(),
                    api_passphrase: None,
                },
                ExchangeConfig {
                    name: "kucoin".to_string(),
                    api_key: "your_kucoin_api_key".to_string(),
                    api_secret: "your_kucoin_api_secret".to_string(),
                    api_passphrase: Some("your_kucoin_passphrase".to_string()),
                },
            ],
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

                // 中间价计算方法
                // Simple: 直接取最优价
                // WeightedByVolume: 按订单量加权平均
                // VolumeThreshold: 基于最近成交买单价值的深度阈值
                mid_price_method_bid: MidPriceMethod::VolumeThreshold,  // Bid 使用深度阈值
                mid_price_method_ask: MidPriceMethod::Simple,           // Ask 直接取最优卖价
                orderbook_depth: 20,         // 获取20档订单簿
                volume_threshold_usdt: 100.0, // 当没有成交历史时使用的默认阈值
            },
        };

        let json = serde_json::to_string_pretty(&example)?;
        fs::write(path, json)?;
        Ok(())
    }
}
