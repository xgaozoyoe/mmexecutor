use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::exchange::*;

type HmacSha256 = Hmac<Sha256>;

const BASE_URL: &str = "https://api.kucoin.com";

#[derive(Debug, Clone)]
pub struct KucoinExchange {
    api_key: String,
    api_secret: String,
    api_passphrase: String,
    client: Client,
}

impl KucoinExchange {
    pub fn new(api_key: String, api_secret: String, api_passphrase: String) -> Self {
        Self {
            api_key,
            api_secret,
            api_passphrase,
            client: Client::new(),
        }
    }

    #[allow(dead_code)]
    fn generate_signature(&self, timestamp: u64, method: &str, endpoint: &str, body: &str) -> String {
        let str_to_sign = format!("{}{}{}{}", timestamp, method, endpoint, body);

        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(str_to_sign.as_bytes());
        let result = mac.finalize();

        base64::engine::general_purpose::STANDARD.encode(result.into_bytes())
    }

    #[allow(dead_code)]
    fn generate_passphrase_signature(&self) -> String {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(self.api_passphrase.as_bytes());
        let result = mac.finalize();

        base64::engine::general_purpose::STANDARD.encode(result.into_bytes())
    }

    #[allow(dead_code)]
    fn get_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }
}

#[async_trait]
impl Exchange for KucoinExchange {
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        // KuCoin uses format: BTC-USDT
        let kucoin_symbol = symbol.replace("USDT", "-USDT")
            .replace("USDC", "-USDC")
            .replace("BTC", "-BTC")
            .replace("ETH", "-ETH");

        let url = format!("{}/api/v1/market/orderbook/level2_{}",
            BASE_URL, if limit.unwrap_or(5) <= 20 { "20" } else { "100" });

        let response = self
            .client
            .get(&url)
            .query(&[("symbol", &kucoin_symbol)])
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
        anyhow::bail!("KuCoin implementation not yet complete - place_limit_order")
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        anyhow::bail!("KuCoin implementation not yet complete - get_account_info")
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>> {
        anyhow::bail!("KuCoin implementation not yet complete - get_open_orders")
    }

    async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>> {
        anyhow::bail!("KuCoin implementation not yet complete - get_all_orders")
    }

    async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
        anyhow::bail!("KuCoin implementation not yet complete - get_my_trades")
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<serde_json::Value> {
        anyhow::bail!("KuCoin implementation not yet complete - cancel_order")
    }

    async fn place_batch_limit_orders(
        &self,
        orders: Vec<crate::exchange::BatchOrder>,
    ) -> Result<Vec<Result<OrderResponse>>> {
        if orders.is_empty() {
            return Ok(Vec::new());
        }

        // KuCoin 支持批量下单，但需要注意最多5个订单
        // 如果超过5个，需要分批处理
        let mut all_results = Vec::new();

        for chunk in orders.chunks(5) {
            let timestamp = Self::get_timestamp();

            // 构建批量订单数据
            let order_list: Vec<serde_json::Value> = chunk
                .iter()
                .map(|order| {
                    // KuCoin 使用 BTC-USDT 格式
                    let kucoin_symbol = order.symbol.replace("USDT", "-USDT")
                        .replace("USDC", "-USDC")
                        .replace("BTC", "-BTC")
                        .replace("ETH", "-ETH");

                    serde_json::json!({
                        "clientOid": format!("{}", uuid::Uuid::new_v4()),
                        "symbol": kucoin_symbol,
                        "type": "limit",
                        "side": order.side.to_lowercase(),
                        "price": order.price.to_string(),
                        "size": order.quantity.to_string(),
                    })
                })
                .collect();

            let body_json = serde_json::json!({
                "orderList": order_list
            });

            let body = serde_json::to_string(&body_json)?;
            let endpoint = "/api/v1/orders/multi";

            let signature = self.generate_signature(timestamp, "POST", endpoint, &body);
            let passphrase_sign = self.generate_passphrase_signature();

            let url = format!("{}{}", BASE_URL, endpoint);

            let response = self
                .client
                .post(&url)
                .header("KC-API-KEY", &self.api_key)
                .header("KC-API-SIGN", signature)
                .header("KC-API-TIMESTAMP", timestamp.to_string())
                .header("KC-API-PASSPHRASE", passphrase_sign)
                .header("KC-API-KEY-VERSION", "2")
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
                .context("Failed to place KuCoin batch orders")?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                anyhow::bail!("KuCoin batch order failed: {}", error_text);
            }

            let response_text = response.text().await?;
            let kucoin_response: serde_json::Value = serde_json::from_str(&response_text)
                .context(format!("Failed to parse KuCoin batch response: {}", response_text))?;

            // KuCoin 返回格式: {"code":"200000","data":{"data":[...]}}
            if let Some(data) = kucoin_response["data"]["data"].as_array() {
                for item in data {
                    all_results.push(Ok(OrderResponse {
                        symbol: chunk[0].symbol.clone(),
                        order_id: item["orderId"].as_str().unwrap_or("").to_string(),
                        order_list_id: 0,
                        price: "0".to_string(),
                        orig_qty: "0".to_string(),
                        order_type: "LIMIT".to_string(),
                        stp_mode: "".to_string(),
                        side: chunk[0].side.clone(),
                        transact_time: timestamp as i64,
                    }));
                }
            }
        }

        Ok(all_results)
    }
}
