use anyhow::Result;
use crate::exchange::Trade;

#[derive(Debug, Clone)]
pub struct TradingStats {
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,

    // 买入统计
    pub total_buy_quantity: f64,      // 总买入数量（基础资产）
    pub total_buy_cost: f64,          // 总买入成本（计价资产）
    pub avg_buy_price: f64,           // 加权平均买入价
    pub buy_count: usize,             // 买入次数

    // 卖出统计
    pub total_sell_quantity: f64,     // 总卖出数量（基础资产）
    pub total_sell_revenue: f64,      // 总卖出收入（计价资产）
    pub avg_sell_price: f64,          // 加权平均卖出价
    pub sell_count: usize,            // 卖出次数

    // 24小时买入统计
    pub buy_24h_quantity: f64,        // 24小时买入数量
    pub buy_24h_cost: f64,            // 24小时买入成本
    pub buy_24h_avg_price: f64,       // 24小时平均买入价
    pub buy_24h_count: usize,         // 24小时买入次数

    // 24小时卖出统计
    pub sell_24h_quantity: f64,       // 24小时卖出数量
    pub sell_24h_revenue: f64,        // 24小时卖出收入
    pub sell_24h_avg_price: f64,      // 24小时平均卖出价
    pub sell_24h_count: usize,        // 24小时卖出次数

    // 净持仓
    pub net_quantity: f64,            // 净持仓 = 买入 - 卖出
    pub net_cost: f64,                // 净成本 = 买入成本 - 卖出收入
    pub avg_cost_price: f64,          // 平均成本价（如果还有持仓）

    // 手续费
    pub total_commission_quote: f64,  // 计价资产手续费
    pub total_commission_base: f64,   // 基础资产手续费

    // 已实现盈亏
    pub realized_pnl: f64,            // 已实现盈亏（不考虑未平仓部分）
}

impl TradingStats {
    pub fn from_trades(trades: &[Trade], symbol: &str, base_asset: &str, quote_asset: &str) -> Result<Self> {
        let mut stats = TradingStats {
            symbol: symbol.to_string(),
            base_asset: base_asset.to_string(),
            quote_asset: quote_asset.to_string(),
            total_buy_quantity: 0.0,
            total_buy_cost: 0.0,
            avg_buy_price: 0.0,
            buy_count: 0,
            total_sell_quantity: 0.0,
            total_sell_revenue: 0.0,
            avg_sell_price: 0.0,
            sell_count: 0,
            buy_24h_quantity: 0.0,
            buy_24h_cost: 0.0,
            buy_24h_avg_price: 0.0,
            buy_24h_count: 0,
            sell_24h_quantity: 0.0,
            sell_24h_revenue: 0.0,
            sell_24h_avg_price: 0.0,
            sell_24h_count: 0,
            net_quantity: 0.0,
            net_cost: 0.0,
            avg_cost_price: 0.0,
            total_commission_quote: 0.0,
            total_commission_base: 0.0,
            realized_pnl: 0.0,
        };

        // 计算24小时前的时间戳（毫秒）
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let time_24h_ago = now_ms - 24 * 60 * 60 * 1000;

        for trade in trades {
            let _price = trade.price.parse::<f64>()?;
            let qty = trade.qty.parse::<f64>()?;
            let quote_qty = trade.quote_qty.parse::<f64>()?;
            let commission = trade.commission.parse::<f64>()?;
            let is_within_24h = trade.time >= time_24h_ago;

            if trade.is_buyer {
                // 买入
                stats.total_buy_quantity += qty;
                stats.total_buy_cost += quote_qty;
                stats.buy_count += 1;

                // 24小时买入统计
                if is_within_24h {
                    stats.buy_24h_quantity += qty;
                    stats.buy_24h_cost += quote_qty;
                    stats.buy_24h_count += 1;
                }

                // 统计手续费
                if trade.commission_asset == quote_asset {
                    stats.total_commission_quote += commission;
                } else if trade.commission_asset == base_asset {
                    stats.total_commission_base += commission;
                }
            } else {
                // 卖出
                stats.total_sell_quantity += qty;
                stats.total_sell_revenue += quote_qty;
                stats.sell_count += 1;

                // 24小时卖出统计
                if is_within_24h {
                    stats.sell_24h_quantity += qty;
                    stats.sell_24h_revenue += quote_qty;
                    stats.sell_24h_count += 1;
                }

                // 统计手续费
                if trade.commission_asset == quote_asset {
                    stats.total_commission_quote += commission;
                } else if trade.commission_asset == base_asset {
                    stats.total_commission_base += commission;
                }
            }
        }

        // 计算加权平均价格
        if stats.total_buy_quantity > 0.0 {
            stats.avg_buy_price = stats.total_buy_cost / stats.total_buy_quantity;
        }

        if stats.total_sell_quantity > 0.0 {
            stats.avg_sell_price = stats.total_sell_revenue / stats.total_sell_quantity;
        }

        // 计算24小时加权平均价格
        if stats.buy_24h_quantity > 0.0 {
            stats.buy_24h_avg_price = stats.buy_24h_cost / stats.buy_24h_quantity;
        }

        if stats.sell_24h_quantity > 0.0 {
            stats.sell_24h_avg_price = stats.sell_24h_revenue / stats.sell_24h_quantity;
        }

        // 计算净持仓
        stats.net_quantity = stats.total_buy_quantity - stats.total_sell_quantity;
        stats.net_cost = stats.total_buy_cost - stats.total_sell_revenue;

        // 计算平均成本价（当前持仓的成本价）
        if stats.net_quantity > 0.0 {
            stats.avg_cost_price = stats.net_cost / stats.net_quantity;
        }

        // 计算已实现盈亏（卖出部分）
        // 使用 FIFO 方法简化：假设卖出的部分按平均买入价计算成本
        if stats.total_sell_quantity > 0.0 && stats.avg_buy_price > 0.0 {
            let sold_cost = stats.total_sell_quantity * stats.avg_buy_price;
            stats.realized_pnl = stats.total_sell_revenue - sold_cost;
        }

        Ok(stats)
    }

    pub fn print_summary(&self) {
        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║           TRADING STATISTICS (from trades)            ║");
        println!("╚════════════════════════════════════════════════════════╝\n");

        println!("📊 Symbol: {}", self.symbol);
        println!("   Base Asset: {} | Quote Asset: {}\n", self.base_asset, self.quote_asset);

        println!("📈 BUY Summary (All Time):");
        println!("   Total Bought:     {:.8} {}", self.total_buy_quantity, self.base_asset);
        println!("   Total Cost:       {:.8} {}", self.total_buy_cost, self.quote_asset);
        println!("   Avg Buy Price:    {:.8} {}", self.avg_buy_price, self.quote_asset);
        println!("   Buy Orders:       {}\n", self.buy_count);

        println!("📉 SELL Summary (All Time):");
        println!("   Total Sold:       {:.8} {}", self.total_sell_quantity, self.base_asset);
        println!("   Total Revenue:    {:.8} {}", self.total_sell_revenue, self.quote_asset);
        println!("   Avg Sell Price:   {:.8} {}", self.avg_sell_price, self.quote_asset);
        println!("   Sell Orders:      {}\n", self.sell_count);

        println!("🕐 Last 24 Hours:");
        println!("   ┌─ BUY:");
        println!("   │  Quantity:       {:.8} {}", self.buy_24h_quantity, self.base_asset);
        println!("   │  Cost:           {:.8} {}", self.buy_24h_cost, self.quote_asset);
        println!("   │  Avg Price:      {:.8} {}", self.buy_24h_avg_price, self.quote_asset);
        println!("   │  Orders:         {}", self.buy_24h_count);
        println!("   │");
        println!("   └─ SELL:");
        println!("      Quantity:       {:.8} {}", self.sell_24h_quantity, self.base_asset);
        println!("      Revenue:        {:.8} {}", self.sell_24h_revenue, self.quote_asset);
        println!("      Avg Price:      {:.8} {}", self.sell_24h_avg_price, self.quote_asset);
        println!("      Orders:         {}\n", self.sell_24h_count);

        println!("💼 NET Position:");
        println!("   Net Quantity:     {:+.8} {}", self.net_quantity, self.base_asset);
        println!("   Net Cost:         {:+.8} {}", self.net_cost, self.quote_asset);
        if self.net_quantity > 0.0 {
            println!("   Avg Cost Price:   {:.8} {} (current holding)",
                self.avg_cost_price, self.quote_asset);
        } else if self.net_quantity < 0.0 {
            println!("   ⚠️  Warning: Net quantity is negative (sold more than bought)");
        } else {
            println!("   Status: No open position");
        }
        println!();

        println!("💰 Fees:");
        println!("   {} Fees:  {:.8}", self.quote_asset, self.total_commission_quote);
        println!("   {} Fees:  {:.8}\n", self.base_asset, self.total_commission_base);

        println!("💵 Realized P&L:");
        let pnl_sign = if self.realized_pnl >= 0.0 { "+" } else { "" };
        println!("   {}{:.8} {} (from {} sold)\n",
            pnl_sign, self.realized_pnl, self.quote_asset, self.total_sell_quantity);

        if self.avg_buy_price > 0.0 && self.avg_sell_price > 0.0 {
            let spread = self.avg_sell_price - self.avg_buy_price;
            let spread_pct = (spread / self.avg_buy_price) * 100.0;
            println!("📊 Spread Analysis:");
            println!("   Avg Spread:       {:+.8} {} ({:+.4}%)",
                spread, self.quote_asset, spread_pct);
        }
    }
}
