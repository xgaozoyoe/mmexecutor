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
        let timestamp = Self::get_timestamp();

        // Gate.io uses format: BTC_USDT
        let gate_symbol = symbol.replace("USDT", "_USDT")
            .replace("USDC", "_USDC")
            .replace("BTC", "_BTC")
            .replace("ETH", "_ETH");

        // Generate unique client order ID
        let client_order_id = format!("t-grid{}", timestamp);

        let body_json = serde_json::json!({
            "text": client_order_id,
            "currency_pair": gate_symbol,
            "type": "limit",
            "account": "spot",
            "side": side.to_lowercase(),
            "amount": quantity.to_string(),
            "price": price.to_string(),
            "time_in_force": "gtc"
        });

        let body = serde_json::to_string(&body_json)?;
        let body_hash = format!("{:x}", sha2::Sha512::digest(body.as_bytes()));

        let url_path = "/api/v4/spot/orders";
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
            .context("Failed to place Gate.io order")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Gate.io order failed: {}", error_text);
        }

        let response_text = response.text().await?;
        let gate_response: serde_json::Value = serde_json::from_str(&response_text)
            .context(format!("Failed to parse Gate.io response: {}", response_text))?;

        // Gate.io returns order details directly
        Ok(OrderResponse {
            symbol: symbol.to_string(),
            order_id: gate_response["id"].as_str().unwrap_or("").to_string(),
            order_list_id: 0,
            price: gate_response["price"].as_str().unwrap_or(&price.to_string()).to_string(),
            orig_qty: gate_response["amount"].as_str().unwrap_or(&quantity.to_string()).to_string(),
            order_type: "LIMIT".to_string(),
            stp_mode: "".to_string(),
            side: side.to_uppercase(),
            transact_time: gate_response["create_time"].as_i64().unwrap_or(timestamp as i64),
        })
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        let timestamp = Self::get_timestamp();
        let url_path = "/api/v4/spot/accounts";
        let body_hash = format!("{:x}", sha2::Sha512::digest(b""));
        let signature = self.generate_signature("GET", url_path, "", &body_hash, timestamp);

        let url = format!("{}{}", BASE_URL, url_path);

        let response = self
            .client
            .get(&url)
            .header("KEY", &self.api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .send()
            .await
            .context("Failed to get account info")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get account info: {}", error_text);
        }

        // Gate.io returns array of balances directly: [{"currency": "BTC", "available": "1.0", "locked": "0.0"}]
        let gate_balances: Vec<serde_json::Value> = response
            .json()
            .await
            .context("Failed to parse Gate.io account balances")?;

        // Convert Gate.io format to unified format
        let balances: Vec<Balance> = gate_balances
            .into_iter()
            .map(|v| Balance {
                asset: v["currency"].as_str().unwrap_or("").to_string(),
                free: v["available"].as_str().unwrap_or("0").to_string(),
                locked: v["locked"].as_str().unwrap_or("0").to_string(),
            })
            .collect();

        Ok(AccountInfo { balances })
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>> {
        let timestamp = Self::get_timestamp();
        let url_path = "/api/v4/spot/open_orders";

        let query_string = if let Some(sym) = symbol {
            let gate_symbol = sym.replace("USDT", "_USDT")
                .replace("USDC", "_USDC")
                .replace("BTC", "_BTC")
                .replace("ETH", "_ETH");
            format!("currency_pair={}", gate_symbol)
        } else {
            String::new()
        };

        let body_hash = format!("{:x}", sha2::Sha512::digest(b""));
        let signature = self.generate_signature("GET", url_path, &query_string, &body_hash, timestamp);

        let url = if query_string.is_empty() {
            format!("{}{}", BASE_URL, url_path)
        } else {
            format!("{}{}?{}", BASE_URL, url_path, query_string)
        };

        let response = self
            .client
            .get(&url)
            .header("KEY", &self.api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .send()
            .await
            .context("Failed to get open orders")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get open orders: {}", error_text);
        }

        let response_text = response.text().await?;

        // Gate.io v4 API returns a wrapper object with "orders" array
        let response_value: serde_json::Value = serde_json::from_str(&response_text)
            .context("Failed to parse open orders response")?;

        // Extract the orders array from the response
        let gate_orders = if response_value.is_array() {
            let arr = response_value.as_array().unwrap();
            if !arr.is_empty() && arr[0].get("orders").is_some() {
                // New format: array containing wrapper object(s) with "orders" field
                // Example: [{"currency_pair":"ZKWASM_USDT","total":7,"orders":[...]}]
                arr[0].get("orders")
                    .and_then(|orders| orders.as_array())
                    .unwrap_or(&vec![])
                    .clone()
            } else {
                // Old format: direct array of orders
                arr.clone()
            }
        } else if let Some(orders) = response_value.get("orders") {
            // Alternative format: single wrapper object with "orders" field
            orders.as_array()
                .context("'orders' field is not an array")?
                .clone()
        } else {
            // Fallback to empty array
            vec![]
        };

        let open_orders: Vec<OpenOrder> = gate_orders
            .into_iter()
            .map(|v| {
                // Gate.io API fields:
                // - amount: 订单总数量
                // - left: 剩余未成交数量
                // - filled_amount: 已成交数量
                // Use filled_amount directly or calculate from amount - left
                let amount: f64 = v["amount"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                let filled_amount: f64 = v["filled_amount"].as_str().unwrap_or("0").parse().unwrap_or(0.0);

                // Fallback to calculate if filled_amount is not available
                let executed = if filled_amount > 0.0 {
                    filled_amount
                } else {
                    let left: f64 = v["left"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    amount - left
                };

                OpenOrder {
                    symbol: v["currency_pair"].as_str().unwrap_or("").replace("_", ""),
                    order_id: v["id"].as_str().unwrap_or("").to_string(),
                    price: v["price"].as_str().unwrap_or("0").to_string(),
                    orig_qty: v["amount"].as_str().unwrap_or("0").to_string(),
                    executed_qty: executed.to_string(),
                    status: v["status"].as_str().unwrap_or("open").to_uppercase(),
                    side: v["side"].as_str().unwrap_or("").to_uppercase(),
                    order_type: v["type"].as_str().unwrap_or("limit").to_uppercase(),
                    time: v["create_time"].as_i64().unwrap_or(0) * 1000,
                }
            })
            .collect();

        Ok(open_orders)
    }

    async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>> {
        let gate_symbol = symbol.replace("USDT", "_USDT")
            .replace("USDC", "_USDC")
            .replace("BTC", "_BTC")
            .replace("ETH", "_ETH");

        let limit = limit.unwrap_or(100);
        let half_limit = limit / 2;

        // Gate.io requires 'status' parameter, so we fetch both open and finished orders
        // Fetch open orders
        let mut all_orders = Vec::new();

        // Fetch finished orders
        let timestamp = Self::get_timestamp();
        let url_path = "/api/v4/spot/orders";
        let query_string = format!("currency_pair={}&limit={}&status=finished", gate_symbol, limit);

        let body_hash = format!("{:x}", sha2::Sha512::digest(b""));
        let signature = self.generate_signature("GET", url_path, &query_string, &body_hash, timestamp);

        let url = format!("{}{}?{}", BASE_URL, url_path, query_string);

        let response = self
            .client
            .get(&url)
            .header("KEY", &self.api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .send()
            .await
            .context("Failed to get finished orders")?;

        if response.status().is_success() {
            if let Ok(gate_orders) = response.json::<Vec<serde_json::Value>>().await {
                for v in gate_orders {
                    let amount: f64 = v["amount"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let filled_amount: f64 = v["filled_amount"].as_str().unwrap_or("0").parse().unwrap_or(0.0);

                    let executed = if filled_amount > 0.0 {
                        filled_amount
                    } else {
                        let left: f64 = v["left"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                        amount - left
                    };

                    all_orders.push(OpenOrder {
                        symbol: v["currency_pair"].as_str().unwrap_or("").replace("_", ""),
                        order_id: v["id"].as_str().unwrap_or("").to_string(),
                        price: v["price"].as_str().unwrap_or("0").to_string(),
                        orig_qty: v["amount"].as_str().unwrap_or("0").to_string(),
                        executed_qty: executed.to_string(),
                        status: v["status"].as_str().unwrap_or("finished").to_uppercase(),
                        side: v["side"].as_str().unwrap_or("").to_uppercase(),
                        order_type: v["type"].as_str().unwrap_or("limit").to_uppercase(),
                        time: v["create_time"].as_i64().unwrap_or(0) * 1000,
                    });
                }
            }
        }

        // Also fetch open orders (which might be partially filled)
        let timestamp2 = Self::get_timestamp();
        let query_string2 = format!("currency_pair={}&limit={}", gate_symbol, half_limit);
        let body_hash2 = format!("{:x}", sha2::Sha512::digest(b""));
        let signature2 = self.generate_signature("GET", "/api/v4/spot/open_orders", &query_string2, &body_hash2, timestamp2);
        let url2 = format!("{}/api/v4/spot/open_orders?{}", BASE_URL, query_string2);

        if let Ok(response2) = self
            .client
            .get(&url2)
            .header("KEY", &self.api_key)
            .header("Timestamp", timestamp2.to_string())
            .header("SIGN", signature2)
            .send()
            .await
        {
            if response2.status().is_success() {
                // Handle Gate.io v4 API response which might be wrapped in an object
                if let Ok(response_text) = response2.text().await {
                    let response_value: serde_json::Value = serde_json::from_str(&response_text).unwrap_or(serde_json::json!([]));

                    let gate_orders = if response_value.is_array() {
                        let arr = response_value.as_array().unwrap();
                        if !arr.is_empty() && arr[0].get("orders").is_some() {
                            // New format: array containing wrapper object(s)
                            arr[0].get("orders")
                                .and_then(|orders| orders.as_array())
                                .unwrap_or(&vec![])
                                .clone()
                        } else {
                            // Old format: direct array
                            arr.clone()
                        }
                    } else if let Some(orders) = response_value.get("orders") {
                        orders.as_array().unwrap_or(&vec![]).clone()
                    } else {
                        vec![]
                    };

                    for v in gate_orders {
                        let amount: f64 = v["amount"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                        let filled_amount: f64 = v["filled_amount"].as_str().unwrap_or("0").parse().unwrap_or(0.0);

                        let executed = if filled_amount > 0.0 {
                            filled_amount
                        } else {
                            let left: f64 = v["left"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                            amount - left
                        };

                        all_orders.push(OpenOrder {
                            symbol: v["currency_pair"].as_str().unwrap_or("").replace("_", ""),
                            order_id: v["id"].as_str().unwrap_or("").to_string(),
                            price: v["price"].as_str().unwrap_or("0").to_string(),
                            orig_qty: v["amount"].as_str().unwrap_or("0").to_string(),
                            executed_qty: executed.to_string(),
                            status: v["status"].as_str().unwrap_or("open").to_uppercase(),
                            side: v["side"].as_str().unwrap_or("").to_uppercase(),
                            order_type: v["type"].as_str().unwrap_or("limit").to_uppercase(),
                            time: v["create_time"].as_i64().unwrap_or(0) * 1000,
                        });
                    }
                }
            }
        }

        Ok(all_orders)
    }

    async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>> {
        let timestamp = Self::get_timestamp();
        let url_path = "/api/v4/spot/my_trades";

        let gate_symbol = symbol.replace("USDT", "_USDT")
            .replace("USDC", "_USDC")
            .replace("BTC", "_BTC")
            .replace("ETH", "_ETH");

        let limit = limit.unwrap_or(100);
        let query_string = format!("currency_pair={}&limit={}", gate_symbol, limit);

        let body_hash = format!("{:x}", sha2::Sha512::digest(b""));
        let signature = self.generate_signature("GET", url_path, &query_string, &body_hash, timestamp);

        let url = format!("{}{}?{}", BASE_URL, url_path, query_string);

        let response = self
            .client
            .get(&url)
            .header("KEY", &self.api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .send()
            .await
            .context("Failed to get my trades")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to get my trades: {}", error_text);
        }

        let gate_trades: Vec<serde_json::Value> = response
            .json()
            .await
            .context("Failed to parse my trades")?;

        let trades: Vec<Trade> = gate_trades
            .into_iter()
            .map(|v| Trade {
                symbol: v["currency_pair"].as_str().unwrap_or("").replace("_", ""),
                id: v["id"].as_str().unwrap_or("").to_string(),
                order_id: v["order_id"].as_str().unwrap_or("").to_string(),
                price: v["price"].as_str().unwrap_or("0").to_string(),
                qty: v["amount"].as_str().unwrap_or("0").to_string(),
                quote_qty: v["role"].as_str().unwrap_or("0").to_string(),
                commission: v["fee"].as_str().unwrap_or("0").to_string(),
                commission_asset: v["fee_currency"].as_str().unwrap_or("").to_string(),
                time: v["create_time"].as_i64().unwrap_or(0) * 1000,
                is_buyer: v["side"].as_str().unwrap_or("") == "buy",
                is_maker: v["role"].as_str().unwrap_or("") == "maker",
            })
            .collect();

        Ok(trades)
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<serde_json::Value> {
        let timestamp = Self::get_timestamp();
        let url_path = format!("/api/v4/spot/orders/{}", order_id);

        let gate_symbol = symbol.replace("USDT", "_USDT")
            .replace("USDC", "_USDC")
            .replace("BTC", "_BTC")
            .replace("ETH", "_ETH");

        let query_string = format!("currency_pair={}", gate_symbol);

        let body_hash = format!("{:x}", sha2::Sha512::digest(b""));
        let signature = self.generate_signature("DELETE", &url_path, &query_string, &body_hash, timestamp);

        let url = format!("{}{}?{}", BASE_URL, url_path, query_string);

        let response = self
            .client
            .delete(&url)
            .header("KEY", &self.api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
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

        // Gate.io has strict rate limits, split into smaller batches
        const BATCH_SIZE: usize = 4; // Conservative batch size to avoid rate limits
        const BATCH_DELAY_MS: u64 = 300; // 300ms delay between batches

        let mut all_results = Vec::new();

        // Process orders in smaller batches
        let total_batches = (orders.len() + BATCH_SIZE - 1) / BATCH_SIZE;

        for (batch_idx, chunk) in orders.chunks(BATCH_SIZE).enumerate() {
            // Add delay between batches (except for first batch)
            if batch_idx > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(BATCH_DELAY_MS)).await;
            }

            if total_batches > 1 {
                println!("  📦 Gate.io batch {}/{} ({} orders)...", batch_idx + 1, total_batches, chunk.len());
            }

            let timestamp = Self::get_timestamp();

            // 构建批量订单数据
            let batch_orders: Vec<serde_json::Value> = chunk
                .iter()
                .enumerate()
                .map(|(idx, order)| {
                    // Gate.io 使用 BTC_USDT 格式
                    let gate_symbol = order.symbol.replace("USDT", "_USDT")
                        .replace("USDC", "_USDC")
                        .replace("BTC", "_BTC")
                        .replace("ETH", "_ETH");

                    // Generate unique client order ID (Gate.io requires 't-' prefix)
                    let client_order_id = format!("t-grid{}_{}", timestamp, batch_idx * BATCH_SIZE + idx);

                    serde_json::json!({
                        "text": client_order_id,
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

                // If rate limited, add longer delay and continue
                if error_text.contains("TOO_MANY_REQUESTS") || error_text.contains("Rate Limit") {
                    eprintln!("⚠️  Gate.io rate limit hit, waiting 2 seconds before retry...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                    // Mark these orders as failed
                    for _ in chunk {
                        all_results.push(Err(anyhow::anyhow!("Rate limit exceeded")));
                    }
                    continue;
                }

                anyhow::bail!("Gate.io batch order failed: {}", error_text);
            }

            let response_text = response.text().await?;

            // Gate.io 批量下单返回的格式需要适配到 OrderResponse
            let gate_responses: Vec<serde_json::Value> = serde_json::from_str(&response_text)
                .context(format!("Failed to parse Gate.io batch response: {}", response_text))?;

            // 将 Gate.io 响应转换为 OrderResponse
            for v in gate_responses {
                all_results.push(Ok(OrderResponse {
                    symbol: v["currency_pair"].as_str().unwrap_or("").to_string(),
                    order_id: v["id"].as_str().unwrap_or("").to_string(),
                    order_list_id: 0,
                    price: v["price"].as_str().unwrap_or("0").to_string(),
                    orig_qty: v["amount"].as_str().unwrap_or("0").to_string(),
                    order_type: "LIMIT".to_string(),
                    stp_mode: "".to_string(),
                    side: v["side"].as_str().unwrap_or("").to_uppercase(),
                    transact_time: v["create_time"].as_i64().unwrap_or(0),
                }));
            }
        }

        Ok(all_results)
    }
}
