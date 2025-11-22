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
        let kucoin_symbol = if symbol.ends_with("USDT") {
            symbol.replace("USDT", "-USDT")
        } else if symbol.ends_with("USDC") {
            symbol.replace("USDC", "-USDC")
        } else if symbol.ends_with("BTC") {
            symbol.replace("BTC", "-BTC")
        } else if symbol.ends_with("ETH") {
            symbol.replace("ETH", "-ETH")
        } else {
            symbol.to_string()
        };

        let url = format!("{}/api/v1/market/orderbook/level2_{}",
            BASE_URL, if limit.unwrap_or(5) <= 20 { "20" } else { "100" });

        let response = self
            .client
            .get(&url)
            .query(&[("symbol", &kucoin_symbol)])
            .send()
            .await
            .context("Failed to get order book")?;

        let kucoin_response: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse KuCoin response")?;

        // KuCoin returns: {"code":"200000","data":{"time":123,"sequence":"123","bids":[...],"asks":[...]}}
        if let Some(data) = kucoin_response.get("data") {
            serde_json::from_value(data.clone())
                .context("Failed to parse order book from KuCoin data")
        } else {
            anyhow::bail!("Invalid KuCoin order book response format")
        }
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
        let timestamp = Self::get_timestamp();

        // KuCoin uses format: BTC-USDT
        let kucoin_symbol = if symbol.ends_with("USDT") {
            symbol.replace("USDT", "-USDT")
        } else if symbol.ends_with("USDC") {
            symbol.replace("USDC", "-USDC")
        } else if symbol.ends_with("BTC") {
            symbol.replace("BTC", "-BTC")
        } else if symbol.ends_with("ETH") {
            symbol.replace("ETH", "-ETH")
        } else {
            symbol.to_string()
        };

        // Round price to 5 decimal places (priceIncrement = 0.00001)
        let rounded_price = (price * 100000.0).round() / 100000.0;
        // Round quantity to 1 decimal place (baseIncrement = 0.1)
        let rounded_quantity = (quantity * 10.0).round() / 10.0;

        let body_json = serde_json::json!({
            "clientOid": format!("{}", uuid::Uuid::new_v4()),
            "symbol": kucoin_symbol,
            "type": "limit",
            "side": side.to_lowercase(),
            "price": format!("{:.5}", rounded_price),
            "size": format!("{:.1}", rounded_quantity),
        });

        let body = serde_json::to_string(&body_json)?;
        let endpoint = "/api/v1/hf/orders";

        let signature = self.generate_signature(timestamp, "POST", endpoint, &body);
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, endpoint);

        let response = self
            .client
            .post(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("Failed to place KuCoin order")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("KuCoin order failed: {}", error_text);
        }

        let response_text = response.text().await?;
        let kucoin_response: serde_json::Value = serde_json::from_str(&response_text)
            .context(format!("Failed to parse KuCoin response: {}", response_text))?;

        // KuCoin HF API returns: {"code":"200000","data":{"orderId":"..."}}
        if let Some(data) = kucoin_response.get("data") {
            if let Some(order_id) = data.get("orderId").and_then(|id| id.as_str()) {
                Ok(OrderResponse {
                    symbol: symbol.to_string(),
                    order_id: order_id.to_string(),
                    order_list_id: 0,
                    price: price.to_string(),
                    orig_qty: quantity.to_string(),
                    order_type: "LIMIT".to_string(),
                    stp_mode: "".to_string(),
                    side: side.to_uppercase(),
                    transact_time: timestamp as i64,
                })
            } else {
                anyhow::bail!("Order ID not found in KuCoin response: {}", response_text)
            }
        } else {
            anyhow::bail!("Invalid KuCoin response format: {}", response_text)
        }
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        let timestamp = Self::get_timestamp();
        let endpoint = "/api/v1/accounts";
        let signature = self.generate_signature(timestamp, "GET", endpoint, "");
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, endpoint);

        let response = self
            .client
            .get(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
            .send()
            .await
            .context("Failed to get account info")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get account info: {}", error_text);
        }

        let kucoin_response: serde_json::Value = response.json().await?;

        // KuCoin returns wrapped response: {"code":"200000","data":[{"id":"xxx","currency":"BTC","type":"trade","balance":"1.0","available":"1.0","holds":"0"}]}
        if let Some(data) = kucoin_response.get("data") {
            let kucoin_balances: Vec<serde_json::Value> = serde_json::from_value(data.clone())
                .context("Failed to parse KuCoin account data")?;

            // Convert KuCoin format to unified format
            // Only include "trade" type accounts for spot trading
            let balances: Vec<Balance> = kucoin_balances
                .into_iter()
                .filter(|v| {
                    // Only include "trade" type accounts (spot trading)
                    v["type"].as_str().unwrap_or("") == "trade"
                })
                .map(|v| Balance {
                    asset: v["currency"].as_str().unwrap_or("").to_string(),
                    free: v["available"].as_str().unwrap_or("0").to_string(),
                    locked: v["holds"].as_str().unwrap_or("0").to_string(),
                })
                .collect();

            Ok(AccountInfo { balances })
        } else {
            anyhow::bail!("Invalid KuCoin response format. Response: {}", serde_json::to_string(&kucoin_response).unwrap_or_else(|_| "Unable to serialize".to_string()))
        }
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>> {
        let timestamp = Self::get_timestamp();
        let mut endpoint = "/api/v1/orders?status=active".to_string();

        if let Some(sym) = symbol {
            let kucoin_symbol = if sym.ends_with("USDT") {
                sym.replace("USDT", "-USDT")
            } else if sym.ends_with("USDC") {
                sym.replace("USDC", "-USDC")
            } else if sym.ends_with("BTC") {
                sym.replace("BTC", "-BTC")
            } else if sym.ends_with("ETH") {
                sym.replace("ETH", "-ETH")
            } else {
                sym.to_string()
            };
            endpoint = format!("{}&symbol={}", endpoint, kucoin_symbol);
        }

        let signature = self.generate_signature(timestamp, "GET", &endpoint, "");
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, endpoint);

        let response = self
            .client
            .get(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
            .send()
            .await
            .context("Failed to get open orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get open orders: {}", error_text);
        }

        let kucoin_response: serde_json::Value = response.json().await?;

        if let Some(data) = kucoin_response.get("data").and_then(|d| d.get("items")) {
            let kucoin_orders: Vec<serde_json::Value> = serde_json::from_value(data.clone())?;

            let open_orders: Vec<OpenOrder> = kucoin_orders
                .into_iter()
                .map(|v| OpenOrder {
                    symbol: v["symbol"].as_str().unwrap_or("").replace("-", ""),
                    order_id: v["id"].as_str().unwrap_or("").to_string(),
                    price: v["price"].as_str().unwrap_or("0").to_string(),
                    orig_qty: v["size"].as_str().unwrap_or("0").to_string(),
                    executed_qty: v["dealSize"].as_str().unwrap_or("0").to_string(),
                    status: if v["isActive"].as_bool().unwrap_or(false) { "OPEN" } else { "CLOSED" }.to_string(),
                    side: v["side"].as_str().unwrap_or("").to_uppercase(),
                    order_type: v["type"].as_str().unwrap_or("limit").to_uppercase(),
                    time: v["createdAt"].as_i64().unwrap_or(0),
                })
                .collect();

            Ok(open_orders)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>> {
        let timestamp = Self::get_timestamp();
        let kucoin_symbol = if symbol.ends_with("USDT") {
            symbol.replace("USDT", "-USDT")
        } else if symbol.ends_with("USDC") {
            symbol.replace("USDC", "-USDC")
        } else if symbol.ends_with("BTC") {
            symbol.replace("BTC", "-BTC")
        } else if symbol.ends_with("ETH") {
            symbol.replace("ETH", "-ETH")
        } else {
            symbol.to_string()
        };

        let endpoint = format!("/api/v1/orders?symbol={}", kucoin_symbol);
        let signature = self.generate_signature(timestamp, "GET", &endpoint, "");
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, endpoint);

        let response = self
            .client
            .get(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
            .send()
            .await
            .context("Failed to get all orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get all orders: {}", error_text);
        }

        let kucoin_response: serde_json::Value = response.json().await?;

        if let Some(data) = kucoin_response.get("data").and_then(|d| d.get("items")) {
            let kucoin_orders: Vec<serde_json::Value> = serde_json::from_value(data.clone())?;

            let all_orders: Vec<OpenOrder> = kucoin_orders
                .into_iter()
                .take(limit.unwrap_or(100) as usize)
                .map(|v| OpenOrder {
                    symbol: v["symbol"].as_str().unwrap_or("").replace("-", ""),
                    order_id: v["id"].as_str().unwrap_or("").to_string(),
                    price: v["price"].as_str().unwrap_or("0").to_string(),
                    orig_qty: v["size"].as_str().unwrap_or("0").to_string(),
                    executed_qty: v["dealSize"].as_str().unwrap_or("0").to_string(),
                    status: if v["isActive"].as_bool().unwrap_or(false) { "OPEN" } else { "CLOSED" }.to_string(),
                    side: v["side"].as_str().unwrap_or("").to_uppercase(),
                    order_type: v["type"].as_str().unwrap_or("limit").to_uppercase(),
                    time: v["createdAt"].as_i64().unwrap_or(0),
                })
                .collect();

            Ok(all_orders)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
        let timestamp = Self::get_timestamp();
        let kucoin_symbol = if symbol.ends_with("USDT") {
            symbol.replace("USDT", "-USDT")
        } else if symbol.ends_with("USDC") {
            symbol.replace("USDC", "-USDC")
        } else if symbol.ends_with("BTC") {
            symbol.replace("BTC", "-BTC")
        } else if symbol.ends_with("ETH") {
            symbol.replace("ETH", "-ETH")
        } else {
            symbol.to_string()
        };

        let endpoint = format!("/api/v1/fills?symbol={}", kucoin_symbol);
        let signature = self.generate_signature(timestamp, "GET", &endpoint, "");
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, endpoint);

        let response = self
            .client
            .get(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
            .send()
            .await
            .context("Failed to get my trades")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get my trades: {}", error_text);
        }

        let kucoin_response: serde_json::Value = response.json().await?;

        if let Some(data) = kucoin_response.get("data").and_then(|d| d.get("items")) {
            let kucoin_trades: Vec<serde_json::Value> = serde_json::from_value(data.clone())?;

            let trades: Vec<Trade> = kucoin_trades
                .into_iter()
                .take(limit.unwrap_or(100) as usize)
                .map(|v| Trade {
                    symbol: v["symbol"].as_str().unwrap_or("").replace("-", ""),
                    id: v["tradeId"].as_str().unwrap_or("").to_string(),
                    order_id: v["orderId"].as_str().unwrap_or("").to_string(),
                    price: v["price"].as_str().unwrap_or("0").to_string(),
                    qty: v["size"].as_str().unwrap_or("0").to_string(),
                    quote_qty: v["funds"].as_str().unwrap_or("0").to_string(),
                    commission: v["fee"].as_str().unwrap_or("0").to_string(),
                    commission_asset: v["feeCurrency"].as_str().unwrap_or("").to_string(),
                    time: v["createdAt"].as_i64().unwrap_or(0),
                    is_buyer: v["side"].as_str().unwrap_or("") == "buy",
                    is_maker: v["liquidity"].as_str().unwrap_or("") == "maker",
                })
                .collect();

            Ok(trades)
        } else {
            Ok(Vec::new())
        }
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> Result<serde_json::Value> {
        let timestamp = Self::get_timestamp();
        let endpoint = format!("/api/v1/orders/{}", order_id);
        let signature = self.generate_signature(timestamp, "DELETE", &endpoint, "");
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, endpoint);

        let response = self
            .client
            .delete(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
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
        let total_batches = (orders.len() + 4) / 5;

        for (batch_idx, chunk) in orders.chunks(5).enumerate() {
            // Add delay between batches (except for first batch)
            if batch_idx > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            if total_batches > 1 {
                println!("  📦 KuCoin batch {}/{} ({} orders)...", batch_idx + 1, total_batches, chunk.len());
            }

            let timestamp = Self::get_timestamp();

            // 构建批量订单数据
            let order_list: Vec<serde_json::Value> = chunk
                .iter()
                .map(|order| {
                    // KuCoin 使用 BTC-USDT 格式
                    // Need to insert hyphen before quote currency (USDT, USDC, BTC, ETH)
                    let kucoin_symbol = if order.symbol.ends_with("USDT") {
                        order.symbol.replace("USDT", "-USDT")
                    } else if order.symbol.ends_with("USDC") {
                        order.symbol.replace("USDC", "-USDC")
                    } else if order.symbol.ends_with("BTC") {
                        order.symbol.replace("BTC", "-BTC")
                    } else if order.symbol.ends_with("ETH") {
                        order.symbol.replace("ETH", "-ETH")
                    } else {
                        order.symbol.clone()
                    };

                    // Round price to 5 decimal places (priceIncrement = 0.00001)
                    let rounded_price = (order.price * 100000.0).round() / 100000.0;
                    // Round quantity to 1 decimal place (baseIncrement = 0.1)
                    let rounded_quantity = (order.quantity * 10.0).round() / 10.0;

                    serde_json::json!({
                        "clientOid": format!("{}", uuid::Uuid::new_v4()),
                        "symbol": kucoin_symbol,
                        "type": "limit",
                        "side": order.side.to_lowercase(),
                        "price": format!("{:.5}", rounded_price),
                        "size": format!("{:.1}", rounded_quantity),
                    })
                })
                .collect();

            let body_json = serde_json::json!({
                "orderList": order_list
            });

            let body = serde_json::to_string(&body_json)?;
            let endpoint = "/api/v1/hf/orders/multi";

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

            // KuCoin HF API 返回格式: {"code":"200","msg":"success","data":[{"orderId":"...","success":true},...]}
            if let Some(data) = kucoin_response.get("data").and_then(|d| d.as_array()) {
                for (idx, item) in data.iter().enumerate() {
                    if idx >= chunk.len() {
                        // Safety check: shouldn't happen but prevent panic
                        break;
                    }

                    // Check if order was successful
                    let is_success = item.get("success").and_then(|s| s.as_bool()).unwrap_or(false);

                    if is_success {
                        if let Some(order_id) = item.get("orderId").and_then(|id| id.as_str()) {
                            all_results.push(Ok(OrderResponse {
                                symbol: chunk[idx].symbol.clone(),
                                order_id: order_id.to_string(),
                                order_list_id: 0,
                                price: chunk[idx].price.to_string(),
                                orig_qty: chunk[idx].quantity.to_string(),
                                order_type: "LIMIT".to_string(),
                                stp_mode: "".to_string(),
                                side: chunk[idx].side.clone(),
                                transact_time: timestamp as i64,
                            }));
                        } else {
                            all_results.push(Err(anyhow::anyhow!("Order ID missing in response")));
                        }
                    } else {
                        // Order failed
                        let error_msg = item.get("msg")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Order placement failed");
                        all_results.push(Err(anyhow::anyhow!("KuCoin order failed: {}", error_msg)));
                    }
                }

                // If response has fewer items than expected, mark remaining as failed
                if data.len() < chunk.len() {
                    for _ in data.len()..chunk.len() {
                        all_results.push(Err(anyhow::anyhow!("Order not in KuCoin response")));
                    }
                }
            } else {
                // No data field in response or not an array
                for _ in chunk {
                    all_results.push(Err(anyhow::anyhow!("Invalid KuCoin response format")));
                }
            }
        }

        Ok(all_results)
    }

    async fn get_deposit_history(
        &self,
        asset: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<DepositWithdrawal>> {
        let timestamp = Self::get_timestamp();
        let endpoint = "/api/v1/deposits";

        // 构建查询参数
        let mut query_params = vec![];
        if let Some(currency) = asset {
            query_params.push(format!("currency={}", currency));
        }
        if let Some(start) = start_time {
            query_params.push(format!("startAt={}", start)); // KuCoin uses milliseconds
        }
        if let Some(end) = end_time {
            query_params.push(format!("endAt={}", end));
        }
        query_params.push("status=SUCCESS".to_string()); // 只获取成功的充值

        let query_string = if query_params.is_empty() {
            String::new()
        } else {
            format!("?{}", query_params.join("&"))
        };

        let full_endpoint = format!("{}{}", endpoint, query_string);
        let signature = self.generate_signature(timestamp, "GET", &full_endpoint, "");
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, full_endpoint);

        let response = self
            .client
            .get(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
            .send()
            .await
            .context("Failed to get deposit history")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get deposit history: {}", error_text);
        }

        let response_text = response.text().await?;
        let kucoin_response: serde_json::Value = serde_json::from_str(&response_text)
            .context(format!("Failed to parse KuCoin response: {}", response_text))?;

        // KuCoin returns: {"code":"200000","data":{"currentPage":1,"pageSize":50,"totalNum":1,"totalPage":1,"items":[...]}}
        if let Some(data) = kucoin_response.get("data") {
            if let Some(items) = data.get("items").and_then(|i| i.as_array()) {
                let result = items
                    .iter()
                    .filter_map(|item| {
                        let currency = item.get("currency")?.as_str()?.to_string();
                        let amount = item.get("amount")?.as_str()?.parse::<f64>().ok()?;
                        let created_at = item.get("createdAt")?.as_i64()?;
                        let status = item.get("status")?.as_str()?.to_string();
                        let address = item.get("address").and_then(|a| a.as_str()).map(|s| s.to_string());

                        Some(DepositWithdrawal {
                            asset: currency,
                            amount,
                            transaction_type: "deposit".to_string(),
                            timestamp: created_at,
                            status,
                            tx_id: address,
                        })
                    })
                    .collect();

                Ok(result)
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_withdrawal_history(
        &self,
        asset: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<DepositWithdrawal>> {
        let timestamp = Self::get_timestamp();
        let endpoint = "/api/v1/withdrawals";

        // 构建查询参数
        let mut query_params = vec![];
        if let Some(currency) = asset {
            query_params.push(format!("currency={}", currency));
        }
        if let Some(start) = start_time {
            query_params.push(format!("startAt={}", start)); // KuCoin uses milliseconds
        }
        if let Some(end) = end_time {
            query_params.push(format!("endAt={}", end));
        }
        query_params.push("status=SUCCESS".to_string()); // 只获取成功的提现

        let query_string = if query_params.is_empty() {
            String::new()
        } else {
            format!("?{}", query_params.join("&"))
        };

        let full_endpoint = format!("{}{}", endpoint, query_string);
        let signature = self.generate_signature(timestamp, "GET", &full_endpoint, "");
        let passphrase_signature = self.generate_passphrase_signature();

        let url = format!("{}{}", BASE_URL, full_endpoint);

        let response = self
            .client
            .get(&url)
            .header("KC-API-KEY", &self.api_key)
            .header("KC-API-SIGN", signature)
            .header("KC-API-TIMESTAMP", timestamp.to_string())
            .header("KC-API-PASSPHRASE", passphrase_signature)
            .header("KC-API-KEY-VERSION", "2")
            .send()
            .await
            .context("Failed to get withdrawal history")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get withdrawal history: {}", error_text);
        }

        let response_text = response.text().await?;
        let kucoin_response: serde_json::Value = serde_json::from_str(&response_text)
            .context(format!("Failed to parse KuCoin response: {}", response_text))?;

        // KuCoin returns: {"code":"200000","data":{"currentPage":1,"pageSize":50,"totalNum":1,"totalPage":1,"items":[...]}}
        if let Some(data) = kucoin_response.get("data") {
            if let Some(items) = data.get("items").and_then(|i| i.as_array()) {
                let result = items
                    .iter()
                    .filter_map(|item| {
                        let currency = item.get("currency")?.as_str()?.to_string();
                        let amount = item.get("amount")?.as_str()?.parse::<f64>().ok()?;
                        let created_at = item.get("createdAt")?.as_i64()?;
                        let status = item.get("status")?.as_str()?.to_string();
                        let wallet_tx_id = item.get("walletTxId").and_then(|a| a.as_str()).map(|s| s.to_string());

                        Some(DepositWithdrawal {
                            asset: currency,
                            amount,
                            transaction_type: "withdrawal".to_string(),
                            timestamp: created_at,
                            status,
                            tx_id: wallet_tx_id,
                        })
                    })
                    .collect();

                Ok(result)
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_transfer_history(
        &self,
        _asset: Option<&str>,
        _start_time: Option<i64>,
        _end_time: Option<i64>,
    ) -> Result<Vec<DepositWithdrawal>> {
        // KuCoin 的内部转账历史 API 比较复杂，需要使用不同的端点
        // 暂时返回空列表，或者返回不支持的错误
        // 如果需要实现，可以使用 /api/v1/accounts/transferable 等端点
        anyhow::bail!("Transfer history not implemented for KuCoin yet")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
