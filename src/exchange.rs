use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// 通用数据结构
#[derive(Debug, Deserialize)]
pub struct OrderBook {
    pub bids: Vec<Vec<String>>,
    pub asks: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct OrderResponse {
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "orderListId", default)]
    pub order_list_id: i64,
    pub price: String,
    #[serde(rename = "origQty")]
    pub orig_qty: String,
    #[serde(rename = "type")]
    pub order_type: String,
    #[serde(rename = "stpMode", default)]
    pub stp_mode: String,
    pub side: String,
    #[serde(rename = "transactTime")]
    pub transact_time: i64,
}

#[derive(Debug, Deserialize)]
pub struct OpenOrder {
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub price: String,
    #[serde(rename = "origQty")]
    pub orig_qty: String,
    #[serde(rename = "executedQty")]
    pub executed_qty: String,
    pub status: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub time: i64,
}

#[derive(Debug, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub price: String,
    pub qty: String,
    #[serde(rename = "quoteQty")]
    pub quote_qty: String,
    pub commission: String,
    #[serde(rename = "commissionAsset")]
    pub commission_asset: String,
    pub time: i64,
    #[serde(rename = "isBuyer")]
    pub is_buyer: bool,
    #[serde(rename = "isMaker")]
    pub is_maker: bool,
}

#[derive(Debug, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: String,
    pub locked: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountInfo {
    pub balances: Vec<Balance>,
}

// 批量订单项
#[derive(Debug, Clone)]
pub struct BatchOrder {
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: f64,
}

// Exchange trait 定义所有交易所需要实现的接口
#[async_trait]
pub trait Exchange: Send + Sync {
    /// 获取订单簿
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook>;

    /// 获取中间价（最优买卖价的平均值）
    async fn get_mid_price(&self, symbol: &str) -> Result<f64>;

    /// 下限价单
    async fn place_limit_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<OrderResponse>;

    /// 批量下限价单
    /// 如果交易所支持批量API，使用批量API；否则循环单个下单
    /// 返回每个订单的结果
    async fn place_batch_limit_orders(
        &self,
        orders: Vec<BatchOrder>,
    ) -> Result<Vec<Result<OrderResponse>>>;

    /// 获取账户信息
    async fn get_account_info(&self) -> Result<AccountInfo>;

    /// 获取当前挂单
    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>>;

    /// 获取所有订单（包括历史）
    async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>>;

    /// 获取我的交易记录
    async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>>;

    /// 取消订单
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<serde_json::Value>;

    /// 从交易对中提取资产名称（基础资产和计价资产）
    fn get_symbol_assets(&self, symbol: &str) -> (String, String) {
        // 默认实现
        if symbol.ends_with("USDT") {
            let base = symbol.strip_suffix("USDT").unwrap_or(symbol);
            (base.to_string(), "USDT".to_string())
        } else if symbol.ends_with("USDC") {
            let base = symbol.strip_suffix("USDC").unwrap_or(symbol);
            (base.to_string(), "USDC".to_string())
        } else if symbol.ends_with("BTC") {
            let base = symbol.strip_suffix("BTC").unwrap_or(symbol);
            (base.to_string(), "BTC".to_string())
        } else if symbol.ends_with("ETH") {
            let base = symbol.strip_suffix("ETH").unwrap_or(symbol);
            (base.to_string(), "ETH".to_string())
        } else {
            let len = symbol.len();
            if len > 4 {
                (symbol[..len-4].to_string(), symbol[len-4..].to_string())
            } else {
                (symbol.to_string(), "USDT".to_string())
            }
        }
    }
}
