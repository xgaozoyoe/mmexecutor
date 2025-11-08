use crate::config::GridConfig;

#[derive(Debug, Clone)]
pub struct GridOrder {
    pub price: f64,
    pub quantity: f64,
    pub side: String,
}

pub struct OrderCalculator;

impl OrderCalculator {
    pub fn calculate_grid_orders(current_price: f64, config: &GridConfig) -> Vec<GridOrder> {
        let mut orders = Vec::new();

        let buy_orders = Self::calculate_buy_orders(current_price, config);
        let sell_orders = Self::calculate_sell_orders(current_price, config);

        orders.extend(buy_orders);
        orders.extend(sell_orders);

        orders
    }

    fn calculate_buy_orders(current_price: f64, config: &GridConfig) -> Vec<GridOrder> {
        let mut orders = Vec::new();
        let lowest_buy_price = current_price * (1.0 - config.buy_price_percentage / 100.0);

        for i in 0..config.grid_levels {
            // 第一个订单使用 first_buy_offset_percentage
            // 后续订单在第一个订单的基础上继续按 grid_interval_percentage 递减
            let price_offset_percentage = config.first_buy_offset_percentage + config.grid_interval_percentage * i as f64;
            let price = current_price * (1.0 - price_offset_percentage / 100.0);

            if price < lowest_buy_price {
                break;
            }

            let weight = Self::calculate_weight(i, config.grid_levels);
            let value = config.total_buy_value * weight;
            let quantity = value / price;

            orders.push(GridOrder {
                price: Self::round_price(price),
                quantity: Self::round_quantity(quantity),
                side: "BUY".to_string(),
            });
        }

        orders
    }

    fn calculate_sell_orders(current_price: f64, config: &GridConfig) -> Vec<GridOrder> {
        let mut orders = Vec::new();
        let highest_sell_price = current_price * (1.0 + config.sell_price_percentage / 100.0);

        for i in 0..config.grid_levels {
            // 第一个订单使用 first_sell_offset_percentage
            // 后续订单在第一个订单的基础上继续按 grid_interval_percentage 递增
            let price_offset_percentage = config.first_sell_offset_percentage + config.grid_interval_percentage * i as f64;
            let price = current_price * (1.0 + price_offset_percentage / 100.0);

            if price > highest_sell_price {
                break;
            }

            let weight = Self::calculate_weight(i, config.grid_levels);
            let quantity = (config.total_sell_value * weight) / price;

            orders.push(GridOrder {
                price: Self::round_price(price),
                quantity: Self::round_quantity(quantity),
                side: "SELL".to_string(),
            });
        }

        orders
    }

    fn calculate_weight(level: usize, total_levels: usize) -> f64 {
        let normalized_distance = (level + 1) as f64 / total_levels as f64;

        let total_weight: f64 = (1..=total_levels)
            .map(|i| {
                let dist = i as f64 / total_levels as f64;
                dist.powi(2)
            })
            .sum();

        normalized_distance.powi(2) / total_weight
    }

    fn round_price(price: f64) -> f64 {
        (price * 100000000.0).round() / 100000000.0  // 保留8位小数
    }

    fn round_quantity(quantity: f64) -> f64 {
        (quantity * 100000.0).round() / 100000.0  // 保留5位小数
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_grid_orders() {
        let config = GridConfig {
            symbol: "BTCUSDT".to_string(),
            first_buy_offset_percentage: 0.5,
            first_sell_offset_percentage: 0.5,
            buy_price_percentage: 5.0,
            sell_price_percentage: 5.0,
            grid_interval_percentage: 0.5,
            total_buy_value: 100.0,
            total_sell_value: 100.0,
            grid_levels: 10,
            minimal_order_value: 10.0,
        };

        let current_price = 50000.0;
        let orders = OrderCalculator::calculate_grid_orders(current_price, &config);

        assert!(!orders.is_empty());

        let buy_orders: Vec<_> = orders.iter().filter(|o| o.side == "BUY").collect();
        let sell_orders: Vec<_> = orders.iter().filter(|o| o.side == "SELL").collect();

        assert!(!buy_orders.is_empty());
        assert!(!sell_orders.is_empty());

        for order in &buy_orders {
            assert!(order.price < current_price);
        }

        for order in &sell_orders {
            assert!(order.price > current_price);
        }
    }
}
