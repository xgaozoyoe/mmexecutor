use anyhow::{Context, Result};
use crate::config::MidPriceMethod;
use crate::exchange::{OrderBook, OpenOrder};

pub struct PriceCalculator;

impl PriceCalculator {
    /// 根据方法分别计算 bid 和 ask 价格，然后取中间价
    pub fn calculate_mid_price(
        order_book: &OrderBook,
        bid_method: &MidPriceMethod,
        ask_method: &MidPriceMethod,
        all_orders: Option<&[OpenOrder]>,
        default_threshold: f64,
    ) -> Result<MidPriceResult> {
        if order_book.bids.is_empty() || order_book.asks.is_empty() {
            anyhow::bail!("Order book is empty");
        }

        // 分别计算 bid 和 ask 价格
        let (bid_price, bid_details) = Self::calculate_bid_price(order_book, bid_method, all_orders, default_threshold)?;
        let (ask_price, ask_details) = Self::calculate_ask_price(order_book, ask_method)?;

        let mid_price = (bid_price + ask_price) / 2.0;

        Ok(MidPriceResult {
            mid_price,
            bid_price,
            ask_price,
            method_description: format!(
                "Bid: {:?}, Ask: {:?}",
                bid_method, ask_method
            ),
            details: format!("{}\n{}", bid_details, ask_details),
        })
    }

    /// 计算 Bid 价格
    fn calculate_bid_price(
        order_book: &OrderBook,
        method: &MidPriceMethod,
        all_orders: Option<&[OpenOrder]>,
        default_threshold: f64,
    ) -> Result<(f64, String)> {
        match method {
            MidPriceMethod::Simple => {
                let price: f64 = order_book.bids[0][0]
                    .parse()
                    .context("Failed to parse best bid")?;
                Ok((price, format!("  BID: Best bid price: {:.6}", price)))
            }
            MidPriceMethod::WeightedByVolume => {
                let (price, volume) = Self::calculate_weighted_price(&order_book.bids)?;
                Ok((price, format!("  BID: Volume-weighted price: {:.6} (volume: {:.2})", price, volume)))
            }
            MidPriceMethod::VolumeThreshold => {
                Self::calculate_bid_with_threshold(order_book, all_orders, default_threshold)
            }
        }
    }

    /// 计算 Ask 价格
    fn calculate_ask_price(
        order_book: &OrderBook,
        method: &MidPriceMethod,
    ) -> Result<(f64, String)> {
        match method {
            MidPriceMethod::Simple => {
                let price: f64 = order_book.asks[0][0]
                    .parse()
                    .context("Failed to parse best ask")?;
                Ok((price, format!("  ASK: Best ask price: {:.6}", price)))
            }
            MidPriceMethod::WeightedByVolume => {
                let (price, volume) = Self::calculate_weighted_price(&order_book.asks)?;
                Ok((price, format!("  ASK: Volume-weighted price: {:.6} (volume: {:.2})", price, volume)))
            }
            MidPriceMethod::VolumeThreshold => {
                // Ask 侧也可以使用阈值，但这里简化为取最优价
                let price: f64 = order_book.asks[0][0]
                    .parse()
                    .context("Failed to parse best ask")?;
                Ok((price, format!("  ASK: Best ask price: {:.6} (VolumeThreshold not applicable for ask)", price)))
            }
        }
    }

    /// 使用深度阈值计算 Bid 价格
    fn calculate_bid_with_threshold(
        order_book: &OrderBook,
        all_orders: Option<&[OpenOrder]>,
        default_threshold: f64,
    ) -> Result<(f64, String)> {
        let best_bid: f64 = order_book.bids[0][0]
            .parse()
            .context("Failed to parse best bid")?;

        // 计算我们布置的买单中已成交的总价值
        let (filled_buy_value, details) = if let Some(orders) = all_orders {
            Self::calculate_filled_buy_orders_value(orders, default_threshold)
        } else {
            (default_threshold, FilledOrderDetails {
                count: 0,
                total_value: default_threshold,
                description: format!("No order history available, using configured threshold ({:.2} USDT)", default_threshold),
            })
        };

        // 找到买单侧累计量达到阈值的价格
        let (threshold_bid, bid_cumulative) =
            Self::find_threshold_price(&order_book.bids, filled_buy_value)?;

        let bid_offset = ((best_bid - threshold_bid) / best_bid * 100.0).abs();

        let details_str = format!(
            "  BID (VolumeThreshold at {:.2} USDT):\n{}\n    Best: {:.6} -> Threshold: {:.6} (depth: {:.2} USDT, offset: {:.3}%)",
            filled_buy_value, details.description, best_bid, threshold_bid, bid_cumulative, bid_offset
        );

        Ok((threshold_bid, details_str))
    }


    /// 计算某一侧的加权平均价格
    fn calculate_weighted_price(orders: &[Vec<String>]) -> Result<(f64, f64)> {
        let mut total_value = 0.0;
        let mut total_volume = 0.0;

        for order in orders {
            if order.len() < 2 {
                continue;
            }

            let price: f64 = order[0].parse().context("Failed to parse price")?;
            let quantity: f64 = order[1].parse().context("Failed to parse quantity")?;
            let value = price * quantity;

            total_value += value;
            total_volume += quantity;
        }

        if total_volume == 0.0 {
            anyhow::bail!("Total volume is zero");
        }

        let weighted_price = total_value / total_volume;
        Ok((weighted_price, total_volume))
    }


    /// 计算最近5个成交的买单的总价值（USDT）
    fn calculate_filled_buy_orders_value(orders: &[OpenOrder], default_threshold: f64) -> (f64, FilledOrderDetails) {
        let mut filled_buy_orders = Vec::new();

        // 1. 筛选买单（BUY）且已成交的订单（executed_qty > 0）
        for order in orders {
            if order.side != "BUY" {
                continue;
            }

            let executed_qty: f64 = order.executed_qty.parse().unwrap_or(0.0);
            if executed_qty <= 0.0 {
                continue;
            }

            let price: f64 = order.price.parse().unwrap_or(0.0);
            let value = price * executed_qty;

            filled_buy_orders.push(FilledOrderInfo {
                price,
                executed_qty,
                value,
                status: order.status.clone(),
                time: order.time,
            });
        }

        if filled_buy_orders.is_empty() {
            return (default_threshold, FilledOrderDetails {
                count: 0,
                total_value: default_threshold,
                description: format!("No filled buy orders found, using configured threshold ({:.2} USDT)", default_threshold),
            });
        }

        // 2. 按时间排序，最新的在前
        filled_buy_orders.sort_by(|a, b| b.time.cmp(&a.time));

        // 3. 取最近的5个
        let recent_count = 5;
        let recent_orders: Vec<_> = filled_buy_orders.iter().take(recent_count).cloned().collect();

        // 4. 计算这5个订单的总价值
        let total_value: f64 = recent_orders.iter().map(|o| o.value).sum();

        // 构建描述信息
        let mut desc = format!(
            "Based on {} most recent filled BUY order(s) (total filled: {}):",
            recent_orders.len(),
            filled_buy_orders.len()
        );

        for (i, order) in recent_orders.iter().enumerate() {
            let time_str = chrono::DateTime::from_timestamp(order.time / 1000, 0)
                .map(|dt| dt.format("%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            desc.push_str(&format!(
                "\n    {}. {} | {:.6} @ {:.6} = {:.2} USDT ({})",
                i + 1, time_str, order.executed_qty, order.price, order.value, order.status
            ));
        }

        desc.push_str(&format!("\n  Threshold value: {:.2} USDT", total_value));

        (total_value, FilledOrderDetails {
            count: recent_orders.len(),
            total_value,
            description: desc,
        })
    }

    /// 找到累计量（USDT）达到阈值时的价格
    /// 返回 (价格, 实际累计量)
    fn find_threshold_price(orders: &[Vec<String>], threshold_usdt: f64) -> Result<(f64, f64)> {
        let mut cumulative_value = 0.0;

        for order in orders {
            if order.len() < 2 {
                continue;
            }

            let price: f64 = order[0].parse().context("Failed to parse price")?;
            let quantity: f64 = order[1].parse().context("Failed to parse quantity")?;
            let value = price * quantity;

            cumulative_value += value;

            if cumulative_value >= threshold_usdt {
                return Ok((price, cumulative_value));
            }
        }

        // 如果所有订单的累计量都没达到阈值，返回最后一个价格和实际累计量
        if let Some(last_order) = orders.last() {
            if !last_order.is_empty() {
                let price = last_order[0]
                    .parse()
                    .context("Failed to parse last price")?;
                return Ok((price, cumulative_value));
            }
        }

        anyhow::bail!("No valid orders found")
    }
}

#[derive(Debug)]
pub struct MidPriceResult {
    pub mid_price: f64,
    pub bid_price: f64,
    pub ask_price: f64,
    pub method_description: String,
    pub details: String,
}

#[derive(Debug, Clone)]
struct FilledOrderInfo {
    price: f64,
    executed_qty: f64,
    value: f64,
    status: String,
    time: i64,
}

#[derive(Debug)]
struct FilledOrderDetails {
    count: usize,
    total_value: f64,
    description: String,
}
