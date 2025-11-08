pub mod mexc;
pub mod gate;
pub mod kucoin;

pub use mexc::MexcExchange;
pub use gate::GateExchange;
pub use kucoin::KucoinExchange;

use crate::exchange::Exchange;
use anyhow::Result;
use std::sync::Arc;

/// 交易所类型枚举
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExchangeType {
    Mexc,
    Gate,
    Kucoin,
}

/// 交易所工厂，根据配置创建对应的交易所实例
pub fn create_exchange(
    exchange_type: &ExchangeType,
    api_key: String,
    api_secret: String,
    api_passphrase: Option<String>,
) -> Result<Arc<dyn Exchange>> {
    match exchange_type {
        ExchangeType::Mexc => {
            Ok(Arc::new(MexcExchange::new(api_key, api_secret)))
        }
        ExchangeType::Gate => {
            Ok(Arc::new(GateExchange::new(api_key, api_secret)))
        }
        ExchangeType::Kucoin => {
            let passphrase = api_passphrase
                .ok_or_else(|| anyhow::anyhow!("KuCoin requires api_passphrase"))?;
            Ok(Arc::new(KucoinExchange::new(api_key, api_secret, passphrase)))
        }
    }
}
