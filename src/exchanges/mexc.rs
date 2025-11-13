use anyhow::{Context, Result};
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::exchange::*;

type HmacSha256 = Hmac<Sha256>;

const BASE_URL: &str = "https://api.mexc.com";

#[derive(Debug, Clone)]
pub struct MexcExchange {
    api_key: String,
    api_secret: String,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct TickerPrice {
    pub symbol: String,
    pub price: String,
}

#[derive(Debug, Serialize)]
pub struct BatchOrderItem {
    pub symbol: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub quantity: String,
    pub price: String,
    #[serde(rename = "timeInForce")]
    pub time_in_force: String,
}

impl MexcExchange {
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
}

#[async_trait]
impl Exchange for MexcExchange {
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
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

    async fn get_mid_price(&self, symbol: &str) -> Result<f64> {
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

    async fn place_limit_order(
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

        let response_text = response.text().await?;
        serde_json::from_str(&response_text)
            .context(format!("Failed to parse order response. Raw response: {}", response_text))
    }

    async fn place_batch_limit_orders(
        &self,
        orders: Vec<crate::exchange::BatchOrder>,
    ) -> Result<Vec<Result<OrderResponse>>> {
        if orders.is_empty() {
            return Ok(Vec::new());
        }

        // 所有订单必须是同一个交易对
        let symbol = &orders[0].symbol;

        let timestamp = Self::get_timestamp();

        // 构建批量订单数据
        let batch_orders: Vec<BatchOrderItem> = orders
            .iter()
            .map(|order| BatchOrderItem {
                symbol: order.symbol.clone(),
                side: order.side.to_uppercase(),
                order_type: "LIMIT".to_string(),
                quantity: order.quantity.to_string(),
                price: order.price.to_string(),
                time_in_force: "GTC".to_string(),
            })
            .collect();

        let batch_orders_json = serde_json::to_string(&batch_orders)?;

        let query_string = format!("batchOrders={}&timestamp={}",
            urlencoding::encode(&batch_orders_json),
            timestamp
        );

        let signature = self.generate_signature(&query_string);
        let url = format!("{}/api/v3/batchOrders?{}&signature={}", BASE_URL, query_string, signature);

        let response = self
            .client
            .post(&url)
            .header("X-MEXC-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to place batch orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Batch order placement failed: {}", error_text);
        }

        let response_text = response.text().await?;

        let responses: Vec<OrderResponse> = serde_json::from_str(&response_text)
            .context(format!("Failed to parse batch order response. Raw response: {}", response_text))?;

        // 将所有成功的响应包装为 Ok
        Ok(responses.into_iter().map(Ok).collect())
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
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

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>> {
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

    async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>> {
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

    async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
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

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<serde_json::Value> {
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

        let response_text = response.text().await?;
        serde_json::from_str(&response_text)
            .context(format!("Failed to parse cancel order response. Raw response: {}", response_text))
    }
}
