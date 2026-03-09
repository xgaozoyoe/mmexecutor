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
    pub first_buy_offset_percentage: f64,   // 第一个买单距离当前价格的百分比
    pub first_sell_offset_percentage: f64,  // 第一个卖单距离当前价格的百分比
    pub buy_price_percentage: f64,
    pub sell_price_percentage: f64,

    pub buy_grid_interval_percentage: f64,  // 买单网格间隔
    pub sell_grid_interval_percentage: f64, // 卖单网格间隔

    pub total_buy_value: f64,
    pub total_sell_value: f64,

    pub buy_grid_levels: usize,  // 买单层数
    pub sell_grid_levels: usize, // 卖单层数

    pub minimal_order_value: f64,           // 最小订单总价值，低于此值不布单
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

/// 刷量账户配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAccountConfig {
    pub api_key: String,
    pub api_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_passphrase: Option<String>,  // KuCoin 需要
}

fn default_volume_value() -> f64 {
    10.0
}

fn default_price_offset() -> f64 {
    0.000001  // 默认价格偏移量
}

fn default_guard_price_offset() -> f64 {
    0.000001  // 阻挡单价格偏移量（相对于maker单）
}

fn default_guard_quantity_percent() -> f64 {
    1.0  // 阻挡单数量百分比（相对于maker单），默认1%
}

fn default_enable_guard_order() -> bool {
    true  // 默认启用阻挡单
}

/// 单个交易所的刷量配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeVolumeConfig {
    pub exchange: String,  // 交易所名称: "mexc", "gate", "kucoin"
    pub account1: VolumeAccountConfig,  // 第一个账户 (maker)
    pub account2: VolumeAccountConfig,  // 第二个账户 (taker)
    #[serde(default = "default_volume_value")]
    pub default_value: f64,  // 默认刷量金额（USDT），默认10
    #[serde(default = "default_price_offset")]
    pub price_offset: f64,  // 价格偏移量（绝对值），相对于最优价的偏移
    #[serde(default = "default_enable_guard_order")]
    pub enable_guard_order: bool,  // 是否启用阻挡单防套利
    #[serde(default = "default_guard_price_offset")]
    pub guard_price_offset: f64,  // 阻挡单相对于maker单的价格偏移
    #[serde(default = "default_guard_quantity_percent")]
    pub guard_quantity_percent: f64,  // 阻挡单数量百分比（相对于maker单的百分比）
}

/// 刷量配置包装器 - 支持单个交易所或多个交易所配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VolumeConfigWrapper {
    /// 多个交易所配置（新格式）
    Multiple(Vec<ExchangeVolumeConfig>),
    /// 单个交易所配置（旧格式，向后兼容）
    Single(ExchangeVolumeConfig),
}

impl VolumeConfigWrapper {
    /// 获取指定交易所的配置，如果不指定则返回第一个
    pub fn get_exchange_config(&self, exchange: Option<&str>) -> Option<&ExchangeVolumeConfig> {
        match self {
            VolumeConfigWrapper::Single(config) => {
                if let Some(ex) = exchange {
                    if config.exchange.to_lowercase() == ex.to_lowercase() {
                        Some(config)
                    } else {
                        None
                    }
                } else {
                    Some(config)
                }
            }
            VolumeConfigWrapper::Multiple(configs) => {
                if let Some(ex) = exchange {
                    configs.iter().find(|c| c.exchange.to_lowercase() == ex.to_lowercase())
                } else {
                    configs.first()
                }
            }
        }
    }

    /// 获取所有可用的交易所名称
    pub fn available_exchanges(&self) -> Vec<&str> {
        match self {
            VolumeConfigWrapper::Single(config) => vec![&config.exchange],
            VolumeConfigWrapper::Multiple(configs) => {
                configs.iter().map(|c| c.exchange.as_str()).collect()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GridConfigWrapper {
    // 条件配置：根据价格选择不同的配置
    Conditional {
        symbol: String,
        price_threshold: f64,
        config_above_threshold: GridConfig,  // 价格 >= threshold 时使用
        config_below_threshold: GridConfig,  // 价格 < threshold 时使用
    },
    // 简单配置：直接使用单一配置
    Simple {
        symbol: String,
        config: GridConfig,
    },
}

impl GridConfigWrapper {
    // 根据当前价格选择合适的配置
    pub fn select_config(&self, current_price: f64) -> GridConfig {
        match self {
            GridConfigWrapper::Conditional {
                price_threshold,
                config_above_threshold,
                config_below_threshold,
                ..
            } => {
                if current_price >= *price_threshold {
                    println!("📊 Price {:.6} >= threshold {:.6}, using config_above_threshold",
                             current_price, price_threshold);
                    config_above_threshold.clone()
                } else {
                    println!("📊 Price {:.6} < threshold {:.6}, using config_below_threshold",
                             current_price, price_threshold);
                    config_below_threshold.clone()
                }
            }
            GridConfigWrapper::Simple { config, .. } => {
                println!("📊 Using simple config (no conditional)");
                config.clone()
            }
        }
    }

    // 获取交易对符号
    pub fn get_symbol(&self) -> &str {
        match self {
            GridConfigWrapper::Conditional { symbol, .. } => symbol,
            GridConfigWrapper::Simple { symbol, .. } => symbol,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub exchanges: Vec<ExchangeConfig>,
    pub grid: GridConfigWrapper,

    // 中间价计算相关配置（条件之外）
    #[serde(default)]
    pub mid_price_method_bid: MidPriceMethod,   // Bid 价格计算方法
    #[serde(default)]
    pub mid_price_method_ask: MidPriceMethod,   // Ask 价格计算方法
    #[serde(default = "default_orderbook_depth")]
    pub orderbook_depth: u32,                   // 获取的订单簿深度
    #[serde(default = "default_volume_threshold")]
    pub volume_threshold_usdt: f64,             // VolumeThreshold 方法的默认阈值（USDT）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_mid_price: Option<f64>,             // 固定中间价，如果设置则直接使用此价格

    // 刷量配置（可选）- 支持单个或多个交易所
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<VolumeConfigWrapper>,
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
        // 验证 symbol
        if config.grid.get_symbol().is_empty() {
            anyhow::bail!("Symbol cannot be empty");
        }

        // 验证网格配置
        let configs_to_validate: Vec<&GridConfig> = match &config.grid {
            GridConfigWrapper::Conditional { config_above_threshold, config_below_threshold, price_threshold, .. } => {
                if *price_threshold <= 0.0 {
                    anyhow::bail!("Price threshold must be positive");
                }
                vec![config_above_threshold, config_below_threshold]
            }
            GridConfigWrapper::Simple { config, .. } => vec![config],
        };

        for grid_config in configs_to_validate {
            if grid_config.first_buy_offset_percentage <= 0.0 {
                anyhow::bail!("First buy offset percentage must be positive");
            }
            if grid_config.first_sell_offset_percentage <= 0.0 {
                anyhow::bail!("First sell offset percentage must be positive");
            }
            if grid_config.buy_price_percentage <= 0.0 || grid_config.buy_price_percentage >= 100.0 {
                anyhow::bail!("Buy price percentage must be between 0 and 100");
            }
            if grid_config.sell_price_percentage <= 0.0 || grid_config.sell_price_percentage >= 100.0 {
                anyhow::bail!("Sell price percentage must be between 0 and 100");
            }
            if grid_config.buy_grid_interval_percentage <= 0.0 {
                anyhow::bail!("Buy grid interval percentage must be positive");
            }
            if grid_config.sell_grid_interval_percentage <= 0.0 {
                anyhow::bail!("Sell grid interval percentage must be positive");
            }
            if grid_config.total_buy_value <= 0.0 {
                anyhow::bail!("Total buy value must be positive");
            }
            if grid_config.total_sell_value <= 0.0 {
                anyhow::bail!("Total sell value must be positive");
            }
            if grid_config.buy_grid_levels == 0 {
                anyhow::bail!("Buy grid levels must be at least 1");
            }
            if grid_config.sell_grid_levels == 0 {
                anyhow::bail!("Sell grid levels must be at least 1");
            }
            if grid_config.minimal_order_value <= 0.0 {
                anyhow::bail!("Minimal order value must be positive");
            }
        }
        Ok(())
    }

    pub fn create_example(path: &str) -> Result<()> {
        // 创建一个简单配置的示例
        Self::create_simple_example(path)
    }

    pub fn create_simple_example(path: &str) -> Result<()> {
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
            grid: GridConfigWrapper::Simple {
                symbol: "BTCUSDT".to_string(),
                config: GridConfig {
                    first_buy_offset_percentage: 0.5,  // 第一个买单距离当前价格 0.5%
                    first_sell_offset_percentage: 0.5, // 第一个卖单距离当前价格 0.5%
                    buy_price_percentage: 5.0,
                    sell_price_percentage: 5.0,
                    buy_grid_interval_percentage: 0.5,   // 买单网格间隔 0.5%
                    sell_grid_interval_percentage: 0.5,  // 卖单网格间隔 0.5%
                    total_buy_value: 500.0,  // 最小建议值，确保每个订单 >= 1 USDT
                    total_sell_value: 500.0,
                    buy_grid_levels: 10,   // 买单层数
                    sell_grid_levels: 10,  // 卖单层数
                    minimal_order_value: 10.0,  // 调整后的总价值低于此值不布单
                },
            },
            // 中间价计算方法（条件之外的全局配置）
            // Simple: 直接取最优价
            // WeightedByVolume: 按订单量加权平均
            // VolumeThreshold: 基于最近成交买单价值的深度阈值
            mid_price_method_bid: MidPriceMethod::VolumeThreshold,  // Bid 使用深度阈值
            mid_price_method_ask: MidPriceMethod::Simple,           // Ask 直接取最优卖价
            orderbook_depth: 20,         // 获取20档订单簿
            volume_threshold_usdt: 100.0, // 当没有成交历史时使用的默认阈值
            fix_mid_price: None,         // 固定中间价（可选，设置后直接使用此价格作为中间价）
            volume: None,  // 刷量配置（可选，使用 create-volume 命令时需要配置）
        };

        let json = serde_json::to_string_pretty(&example)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn create_conditional_example(path: &str) -> Result<()> {
        let base_config = GridConfig {
            first_buy_offset_percentage: 0.5,
            first_sell_offset_percentage: 0.5,
            buy_price_percentage: 5.0,
            sell_price_percentage: 5.0,
            buy_grid_interval_percentage: 0.3,
            sell_grid_interval_percentage: 0.3,
            total_buy_value: 1500.0,
            total_sell_value: 1500.0,
            buy_grid_levels: 7,
            sell_grid_levels: 7,
            minimal_order_value: 100.0,
        };

        let example = Config {
            exchanges: vec![
                ExchangeConfig {
                    name: "mexc".to_string(),
                    api_key: "your_mexc_api_key".to_string(),
                    api_secret: "your_mexc_api_secret".to_string(),
                    api_passphrase: None,
                },
            ],
            grid: GridConfigWrapper::Conditional {
                symbol: "ZKWASMUSDT".to_string(),
                price_threshold: 0.01,  // 价格阈值：0.01 USDT
                config_above_threshold: GridConfig {
                    // 高价时：密集买单，稀疏卖单
                    buy_grid_levels: 10,
                    sell_grid_levels: 5,
                    buy_grid_interval_percentage: 0.2,  // 买单间隔更小
                    sell_grid_interval_percentage: 0.5,  // 卖单间隔更大
                    ..base_config.clone()
                },
                config_below_threshold: GridConfig {
                    // 低价时：稀疏买单，密集卖单
                    buy_grid_levels: 5,
                    sell_grid_levels: 10,
                    buy_grid_interval_percentage: 0.5,  // 买单间隔更大
                    sell_grid_interval_percentage: 0.2,  // 卖单间隔更小
                    ..base_config
                },
            },
            // 中间价计算方法（条件之外的全局配置）
            mid_price_method_bid: MidPriceMethod::VolumeThreshold,
            mid_price_method_ask: MidPriceMethod::Simple,
            orderbook_depth: 20,
            volume_threshold_usdt: 100.0,
            fix_mid_price: None,  // 固定中间价（可选）
            volume: None,
        };

        let json = serde_json::to_string_pretty(&example)?;
        fs::write(path, json)?;
        Ok(())
    }
}
