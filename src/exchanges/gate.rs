use anyhow::{Context, Result};
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::{Sha512, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::exchange::*;

type HmacSha512 = Hmac<Sha512>;

const BASE_URL: &str = "https://api.gateio.ws";

#[derive(Debug, Clone)]
pub struct GateExchange {
    api_key: String,
    api_secret: String,
    client: Client,
}

impl GateExchange {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            client: Client::new(),
        }
    }

    #[allow(dead_code)]
    fn generate_signature(&self, method: &str, url_path: &str, query_string: &str, body_hash: &str, timestamp: u64) -> String {
        let payload = format!("{}\n{}\n{}\n{}\n{}", method, url_path, query_string, body_hash, timestamp);

        let mut mac = HmacSha512::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    #[allow(dead_code)]
    fn get_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    }
}

#[async_trait]
impl Exchange for GateExchange {
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        // Gate.io uses different symbol format: BTC_USDT instead of BTCUSDT
        let gate_symbol = symbol.replace("USDT", "_USDT")
            .replace("USDC", "_USDC")
            .replace("BTC", "_BTC")
            .replace("ETH", "_ETH");

        let url = format!("{}/api/v4/spot/order_book", BASE_URL);
        let limit = limit.unwrap_or(5);

        let response = self
            .client
            .get(&url)
            .query(&[("currency_pair", &gate_symbol), ("limit", &limit.to_string())])
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

        Ok((highest_bid + lowest_ask) / 2.0)
    }

    async fn place_limit_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: f64,
    ) -> Result<OrderResponse> {
        anyhow::bail!("Gate.io implementation not yet complete - place_limit_order")
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        anyhow::bail!("Gate.io implementation not yet complete - get_account_info")
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>> {
        anyhow::bail!("Gate.io implementation not yet complete - get_open_orders")
    }

    async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>> {
        anyhow::bail!("Gate.io implementation not yet complete - get_all_orders")
    }

    async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
        anyhow::bail!("Gate.io implementation not yet complete - get_my_trades")
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<serde_json::Value> {
        anyhow::bail!("Gate.io implementation not yet complete - cancel_order")
    }

    async fn place_batch_limit_orders(
        &self,
        orders: Vec<crate::exchange::BatchOrder>,
    ) -> Result<Vec<Result<OrderResponse>>> {
        if orders.is_empty() {
            return Ok(Vec::new());
        }

        let timestamp = Self::get_timestamp();

        // 构建批量订单数据
        let batch_orders: Vec<serde_json::Value> = orders
            .iter()
            .map(|order| {
                // Gate.io 使用 BTC_USDT 格式
                let gate_symbol = order.symbol.replace("USDT", "_USDT")
                    .replace("USDC", "_USDC")
                    .replace("BTC", "_BTC")
                    .replace("ETH", "_ETH");

                serde_json::json!({
                    "currency_pair": gate_symbol,
                    "type": "limit",
                    "account": "spot",
                    "side": order.side.to_lowercase(),
                    "amount": order.quantity.to_string(),
                    "price": order.price.to_string(),
                    "time_in_force": "gtc"
                })
            })
            .collect();

        let body = serde_json::to_string(&batch_orders)?;
        let body_hash = format!("{:x}", sha2::Sha512::digest(body.as_bytes()));

        let url_path = "/api/v4/spot/batch_orders";
        let signature = self.generate_signature("POST", url_path, "", &body_hash, timestamp);

        let url = format!("{}{}", BASE_URL, url_path);

        let response = self
            .client
            .post(&url)
            .header("KEY", &self.api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("Failed to place batch orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Gate.io batch order failed: {}", error_text);
        }

        let response_text = response.text().await?;

        // Gate.io 批量下单返回的格式需要适配到 OrderResponse
        // 这里简化处理，返回成功
        let gate_responses: Vec<serde_json::Value> = serde_json::from_str(&response_text)
            .context(format!("Failed to parse Gate.io batch response: {}", response_text))?;

        // 将 Gate.io 响应转换为 OrderResponse
        let results: Vec<Result<OrderResponse>> = gate_responses
            .into_iter()
            .map(|v| {
                Ok(OrderResponse {
                    symbol: v["currency_pair"].as_str().unwrap_or("").to_string(),
                    order_id: v["id"].as_str().unwrap_or("").to_string(),
                    order_list_id: 0,
                    price: v["price"].as_str().unwrap_or("0").to_string(),
                    orig_qty: v["amount"].as_str().unwrap_or("0").to_string(),
                    order_type: "LIMIT".to_string(),
                    stp_mode: "".to_string(),
                    side: v["side"].as_str().unwrap_or("").to_uppercase(),
                    transact_time: v["create_time"].as_i64().unwrap_or(0),
                })
            })
            .collect();

        Ok(results)
    }
}
