use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

const BASE_URL: &str = "https://api.mexc.com";

#[derive(Debug, Clone)]
pub struct MexcClient {
    api_key: String,
    api_secret: String,
    client: Client,
}

#[derive(Debug, Deserialize)]
pub struct TickerPrice {
    pub symbol: String,
    pub price: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderBook {
    pub bids: Vec<Vec<String>>,  // [[price, quantity], ...]
    pub asks: Vec<Vec<String>>,  // [[price, quantity], ...]
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub symbol: String,
    pub side: String,
    pub r#type: String,
    pub quantity: String,
    pub price: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderResponse {
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: String,  // MEXC 返回字符串类型的订单 ID
    #[serde(rename = "orderListId")]
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
    pub order_id: String,  // MEXC 返回字符串类型的订单 ID
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
    pub id: String,  // MEXC 返回字符串类型的交易 ID
    #[serde(rename = "orderId")]
    pub order_id: String,  // MEXC 返回字符串类型的订单 ID
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

impl MexcClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            client: Client::new(),
        }
    }

    fn generate_signature(&self, query_string: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    fn get_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    pub async fn get_ticker_price(&self, symbol: &str) -> Result<f64> {
        let url = format!("{}/api/v3/ticker/price", BASE_URL);
        let response = self
            .client
            .get(&url)
            .query(&[("symbol", symbol)])
            .send()
            .await
            .context("Failed to get ticker price")?;

        let ticker: TickerPrice = response
            .json()
            .await
            .context("Failed to parse ticker price")?;

        ticker
            .price
            .parse::<f64>()
            .context("Failed to parse price as f64")
    }

    pub async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let url = format!("{}/api/v3/depth", BASE_URL);
        let limit = limit.unwrap_or(5);

        let response = self
            .client
            .get(&url)
            .query(&[("symbol", symbol), ("limit", &limit.to_string())])
            .send()
            .await
            .context("Failed to get order book")?;

        response
            .json()
            .await
            .context("Failed to parse order book")
    }

    pub async fn get_mid_price(&self, symbol: &str) -> Result<f64> {
        let order_book = self.get_order_book(symbol, Some(1)).await?;

        if order_book.bids.is_empty() || order_book.asks.is_empty() {
            anyhow::bail!("Order book is empty");
        }

        let highest_bid: f64 = order_book.bids[0][0]
            .parse()
            .context("Failed to parse highest bid")?;

        let lowest_ask: f64 = order_book.asks[0][0]
            .parse()
            .context("Failed to parse lowest ask")?;

        let mid_price = (highest_bid + lowest_ask) / 2.0;
        Ok(mid_price)
    }

    pub async fn place_limit_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<OrderResponse> {
        let timestamp = Self::get_timestamp();

        let mut params = HashMap::new();
        params.insert("symbol", symbol.to_string());
        params.insert("side", side.to_string());
        params.insert("type", "LIMIT".to_string());
        params.insert("quantity", quantity.to_string());
        params.insert("price", price.to_string());
        params.insert("timestamp", timestamp.to_string());
        params.insert("timeInForce", "GTC".to_string());

        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = self.generate_signature(&query_string);
        let url = format!("{}/api/v3/order?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .post(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to place order")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Order placement failed: {}", error_text);
        }

        // Get the response as text first to see the actual format
        let response_text = response.text().await?;

        // Try to parse as JSON
        serde_json::from_str(&response_text)
            .context(format!("Failed to parse order response. Raw response: {}", response_text))
    }

    pub async fn get_account_info(&self) -> Result<AccountInfo> {
        let timestamp = Self::get_timestamp();
        let query_string = format!("timestamp={}", timestamp);
        let signature = self.generate_signature(&query_string);

        let url = format!("{}/api/v3/account?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .get(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to get account info")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get account info: {}", error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse account info")
    }

    pub fn get_symbol_assets(symbol: &str) -> (String, String) {
        // 从交易对中提取基础资产和计价资产
        // 例如：BTCUSDT -> (BTC, USDT)
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
            // 默认假设最后 4 个字符是计价资产
            let len = symbol.len();
            if len > 4 {
                (symbol[..len-4].to_string(), symbol[len-4..].to_string())
            } else {
                (symbol.to_string(), "USDT".to_string())
            }
        }
    }

    pub async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>> {
        let timestamp = Self::get_timestamp();
        let mut query_string = format!("timestamp={}", timestamp);

        if let Some(sym) = symbol {
            query_string = format!("symbol={}&{}", sym, query_string);
        }

        let signature = self.generate_signature(&query_string);
        let url = format!("{}/api/v3/openOrders?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .get(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to get open orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get open orders: {}", error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse open orders")
    }

    pub async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>> {
        let timestamp = Self::get_timestamp();
        let limit = limit.unwrap_or(100);
        let query_string = format!("symbol={}&timestamp={}&limit={}", symbol, timestamp, limit);
        let signature = self.generate_signature(&query_string);

        let url = format!("{}/api/v3/allOrders?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .get(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to get all orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get all orders: {}", error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse all orders")
    }

    pub async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
        let timestamp = Self::get_timestamp();
        let limit = limit.unwrap_or(500);
        let query_string = format!("symbol={}&timestamp={}&limit={}", symbol, timestamp, limit);
        let signature = self.generate_signature(&query_string);

        let url = format!("{}/api/v3/myTrades?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .get(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to get trades")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get trades: {}", error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse trades")
    }

    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<serde_json::Value> {
        let timestamp = Self::get_timestamp();
        let query_string = format!("symbol={}&orderId={}&timestamp={}", symbol, order_id, timestamp);
        let signature = self.generate_signature(&query_string);

        let url = format!("{}/api/v3/order?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .delete(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to cancel order")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to cancel order: {}", error_text);
        }

        // Get the response as text first to see the actual format
        let response_text = response.text().await?;

        // Try to parse as JSON
        serde_json::from_str(&response_text)
            .context(format!("Failed to parse cancel order response. Raw response: {}", response_text))
    }

    pub async fn cancel_all_orders(&self, symbol: &str) -> Result<serde_json::Value> {
        let timestamp = Self::get_timestamp();
        let query_string = format!("symbol={}&timestamp={}", symbol, timestamp);
        let signature = self.generate_signature(&query_string);

        let url = format!("{}/api/v3/openOrders?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .delete(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to cancel all orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to cancel all orders: {}", error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse cancel all orders response")
    }
}
