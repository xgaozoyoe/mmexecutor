mod config;
mod exchange;
mod exchanges;
mod order_calculator;
mod state;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use exchange::Exchange;
use exchanges::{create_exchange, ExchangeType};
use order_calculator::OrderCalculator;
use state::TradingState;
use std::io::Write;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "mexc-grid-trader")]
#[command(about = "MEXC 网格交易机器人", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 创建示例配置文件
    CreateConfig {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        path: String,
    },
    /// 执行网格布单
    Place {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
    },
    /// 查询当前挂单
    Orders {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
        /// 是否显示所有交易对的挂单
        #[arg(short, long)]
        all: bool,
        /// 显示最近关闭的订单数量（默认 20，设为 0 则不显示）
        #[arg(short, long, default_value = "20")]
        closed: u32,
    },
    /// 查询历史交易记录
    Trades {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
        /// 返回的交易记录数量
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// 取消所有挂单
    Cancel {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
        /// 强制取消，不提示确认
        #[arg(short, long)]
        force: bool,
    },
    /// 持续布单模式，定期自动检查并布单
    Watch {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
        /// 检查间隔（秒），默认 120 秒（2分钟）
        #[arg(short, long, default_value = "120")]
        interval: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::CreateConfig { path } => {
            Config::create_example(&path)?;
            println!("Example config created at: {}", path);
            println!("Please edit the config file with your API credentials and preferences.");
        }
        Commands::Place { config } => {
            place_orders(&config).await?;
        }
        Commands::Orders { config, all, closed } => {
            show_open_orders(&config, all, closed).await?;
        }
        Commands::Trades { config, limit } => {
            show_trades(&config, Some(limit)).await?;
        }
        Commands::Cancel { config, force } => {
            cancel_all_orders(&config, force).await?;
        }
        Commands::Watch { config, interval } => {
            watch_and_place(&config, interval).await?;
        }
    }

    Ok(())
}

async fn show_account_info(client: &Arc<dyn Exchange>, symbol: &str) -> Result<()> {
    println!("📊 Fetching account info...");
    let account = client.get_account_info().await?;

    let (base_asset, quote_asset) = client.get_symbol_assets(symbol);

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║                   ACCOUNT BALANCE                      ║");
    println!("╚════════════════════════════════════════════════════════╝");

    // 找到相关的资产余额
    let mut found_assets = false;

    for balance in &account.balances {
        if balance.asset == base_asset || balance.asset == quote_asset {
            let free: f64 = balance.free.parse().unwrap_or(0.0);
            let locked: f64 = balance.locked.parse().unwrap_or(0.0);
            let total = free + locked;

            if total > 0.0 || balance.asset == base_asset || balance.asset == quote_asset {
                println!("  {} {:<8}  Free: {:>15.8}  Locked: {:>15.8}  Total: {:>15.8}",
                         if balance.asset == base_asset { "🪙" } else { "💵" },
                         balance.asset,
                         free,
                         locked,
                         total);
                found_assets = true;
            }
        }
    }

    if !found_assets {
        println!("  ⚠️  No balance found for {} or {}", base_asset, quote_asset);
    }

    println!("════════════════════════════════════════════════════════\n");

    Ok(())
}

async fn watch_and_place(config_path: &str, interval: u64) -> Result<()> {
    println!("🔄 Starting continuous order placement mode");
    println!("📅 Interval: {} seconds ({} minutes)", interval, interval / 60);
    println!("⚠️  Press Ctrl+C to stop\n");
    println!("{}", "═".repeat(60));

    let mut iteration = 0;

    loop {
        iteration += 1;
        let now = chrono::Utc::now();
        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║  Iteration #{:<45} ║", iteration);
        println!("║  Time: {:<47} ║", now.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("╚════════════════════════════════════════════════════════╝\n");

        // 执行布单（自动模式，无需确认）
        match place_orders_internal(config_path, true).await {
            Ok(_) => println!("\n✅ Iteration #{} completed successfully", iteration),
            Err(e) => println!("\n❌ Iteration #{} failed: {}", iteration, e),
        }

        println!("\n⏳ Waiting {} seconds until next check...", interval);
        println!("   Next run at: {}", (now + chrono::Duration::seconds(interval as i64)).format("%H:%M:%S UTC"));
        println!("{}", "═".repeat(60));

        tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
    }
}

async fn place_orders(config_path: &str) -> Result<()> {
    place_orders_internal(config_path, false).await
}

async fn place_orders_internal(config_path: &str, auto_mode: bool) -> Result<()> {
    println!("Loading config from: {}", config_path);
    let config = Config::from_file(config_path)?;

    // 解析交易所类型
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", config.exchange))
        .context(format!("Invalid exchange type: {}", config.exchange))?;

    println!("Exchange: {:?}", exchange_type);

    // 创建对应的交易所客户端
    let client = create_exchange(
        &exchange_type,
        config.api_key.clone(),
        config.api_secret.clone(),
        config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &config.grid.symbol).await?;

    println!("Fetching order book for {}...", config.grid.symbol);

    // 获取订单簿的最优买卖价
    let order_book = client.get_order_book(&config.grid.symbol, Some(1)).await?;

    if order_book.bids.is_empty() || order_book.asks.is_empty() {
        anyhow::bail!("Order book is empty, cannot calculate mid price");
    }

    let highest_bid: f64 = order_book.bids[0][0]
        .parse()
        .context("Failed to parse highest bid")?;

    let lowest_ask: f64 = order_book.asks[0][0]
        .parse()
        .context("Failed to parse lowest ask")?;

    let current_price = (highest_bid + lowest_ask) / 2.0;

    println!("📊 Order Book:");
    println!("  Highest Bid: {:.6}", highest_bid);
    println!("  Lowest Ask:  {:.6}", lowest_ask);
    println!("  Mid Price:   {:.6}", current_price);

    // 读取上次布单的状态
    let state_file = format!(".state_{}.json", config.grid.symbol);
    let last_state = TradingState::load(&state_file)?;

    if let Some(state) = &last_state {
        println!("\n📊 Previous trading state:");
        println!("  Last price: {:.6}", state.last_price);
        println!("  Last update: {}", chrono::DateTime::<chrono::Utc>::from_timestamp(state.last_update_time, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M:%S UTC"));

        // 如果当前价格低于上次布单价格，清空所有买单
        if current_price < state.last_price {
            let price_drop = ((state.last_price - current_price) / state.last_price) * 100.0;
            println!("\n⚠️  Price dropped by {:.2}% since last placement!", price_drop);
            println!("📉 Current: {:.6} < Last: {:.6}", current_price, state.last_price);
            println!("\n🗑️  Canceling all BUY orders...");

            let all_orders = client.get_open_orders(Some(&config.grid.symbol)).await?;
            let buy_orders: Vec<_> = all_orders.iter().filter(|o| o.side == "BUY").collect();

            if !buy_orders.is_empty() {
                println!("Found {} buy orders to cancel", buy_orders.len());

                for (index, order) in buy_orders.iter().enumerate() {
                    let price: f64 = order.price.parse().unwrap_or(0.0);
                    print!("  [{}/{}] Canceling buy order at {:.6}... ",
                           index + 1, buy_orders.len(), price);
                    std::io::stdout().flush()?;

                    match client.cancel_order(&order.symbol, &order.order_id).await {
                        Ok(_) => println!("✅"),
                        Err(e) => println!("❌ {}", e),
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                println!("✅ All buy orders cancelled");
            } else {
                println!("No buy orders to cancel");
            }
        } else {
            let price_change = ((current_price - state.last_price) / state.last_price) * 100.0;
            if price_change > 0.0 {
                println!("📈 Price increased by {:.2}% since last placement", price_change);
            } else {
                println!("➡️  Price unchanged since last placement");
            }
        }
    } else {
        println!("\n📝 First time placing orders (no previous state found)");
    }

    println!("\n📋 Calculating grid orders...");
    let mut orders = OrderCalculator::calculate_grid_orders(current_price, &config.grid);

    // 获取现有挂单
    println!("\n🔍 Checking existing orders...");
    let existing_orders = client.get_open_orders(Some(&config.grid.symbol)).await?;

    if !existing_orders.is_empty() {
        println!("Found {} existing orders", existing_orders.len());

        // 计算新布单的价格范围
        let buy_orders: Vec<_> = orders.iter().filter(|o| o.side == "BUY").collect();
        let sell_orders: Vec<_> = orders.iter().filter(|o| o.side == "SELL").collect();

        let lowest_new_buy = buy_orders.iter().map(|o| o.price).fold(f64::INFINITY, f64::min);
        let highest_new_buy = buy_orders.iter().map(|o| o.price).fold(0.0, f64::max);
        let lowest_new_sell = sell_orders.iter().map(|o| o.price).fold(f64::INFINITY, f64::min);
        let highest_new_sell = sell_orders.iter().map(|o| o.price).fold(0.0, f64::max);

        println!("  Buy range: {:.6} - {:.6}", lowest_new_buy, highest_new_buy);
        println!("  Sell range: {:.6} - {:.6}", lowest_new_sell, highest_new_sell);

        // 分类现有订单
        let mut orders_to_cancel = Vec::new();
        let mut orders_in_range = Vec::new();

        for existing in &existing_orders {
            let price: f64 = existing.price.parse().unwrap_or(0.0);
            let side = &existing.side;

            let in_range = if side == "BUY" {
                price >= lowest_new_buy && price <= highest_new_buy
            } else if side == "SELL" {
                price >= lowest_new_sell && price <= highest_new_sell
            } else {
                false
            };

            if in_range {
                orders_in_range.push(existing);
            } else {
                orders_to_cancel.push(existing);
            }
        }

        // 取消超出范围的订单
        if !orders_to_cancel.is_empty() {
            println!("\n⚠️  Found {} orders outside the new grid range", orders_to_cancel.len());
            println!("Canceling out-of-range orders...");

            for order in &orders_to_cancel {
                let price: f64 = order.price.parse().unwrap_or(0.0);
                print!("  Canceling {} order at {:.6}... ", order.side, price);
                std::io::stdout().flush()?;

                match client.cancel_order(&order.symbol, &order.order_id).await {
                    Ok(_) => println!("✅"),
                    Err(e) => println!("❌ {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        // 计算范围内老订单的总价值
        if !orders_in_range.is_empty() {
            println!("\n📍 Found {} existing orders within the new grid range", orders_in_range.len());

            let mut existing_buy_value = 0.0;
            let mut existing_sell_value = 0.0;

            for existing in &orders_in_range {
                let price: f64 = existing.price.parse().unwrap_or(0.0);
                let qty: f64 = existing.orig_qty.parse().unwrap_or(0.0);
                let value = price * qty;

                if existing.side == "BUY" {
                    existing_buy_value += value;
                } else if existing.side == "SELL" {
                    existing_sell_value += value;
                }
            }

            println!("  Existing buy orders total value: {:.2} USDT", existing_buy_value);
            println!("  Existing sell orders total value: {:.2} USDT", existing_sell_value);

            // 调整配置的总价值
            let mut adjusted_config = config.grid.clone();
            let remaining_buy_value = (config.grid.total_buy_value - existing_buy_value).max(0.0);
            let remaining_sell_value = (config.grid.total_sell_value - existing_sell_value).max(0.0);

            // 如果调整后的价值小于最小订单价值，设置为 0（不布单）
            if remaining_buy_value < config.grid.minimal_order_value {
                adjusted_config.total_buy_value = 0.0;
                println!("  Adjusted buy value {:.2} USDT < minimal {:.2} USDT, skipping buy orders",
                         remaining_buy_value, config.grid.minimal_order_value);
            } else {
                adjusted_config.total_buy_value = remaining_buy_value;
                println!("  Adjusted buy value for new orders: {:.2} USDT", adjusted_config.total_buy_value);
            }

            if remaining_sell_value < config.grid.minimal_order_value {
                adjusted_config.total_sell_value = 0.0;
                println!("  Adjusted sell value {:.2} USDT < minimal {:.2} USDT, skipping sell orders",
                         remaining_sell_value, config.grid.minimal_order_value);
            } else {
                adjusted_config.total_sell_value = remaining_sell_value;
                println!("  Adjusted sell value for new orders: {:.2} USDT", adjusted_config.total_sell_value);
            }

            // 重新计算新订单
            if adjusted_config.total_buy_value > 0.0 || adjusted_config.total_sell_value > 0.0 {
                orders = OrderCalculator::calculate_grid_orders(current_price, &adjusted_config);
                println!("  Recalculated {} new orders based on remaining budget", orders.len());
            } else {
                orders.clear();
                println!("  No budget remaining for new orders");
            }
        }
    } else {
        println!("No existing orders found");
    }

    // 如果没有新订单需要下单，直接返回
    if orders.is_empty() {
        println!("\n✅ All required orders are already placed. No new orders needed.");
        return Ok(());
    }

    // 过滤掉价值小于 5 USDT 的订单
    const MIN_ORDER_VALUE: f64 = 5.0;
    let original_count = orders.len();

    orders.retain(|order| {
        let value = order.price * order.quantity;
        value >= MIN_ORDER_VALUE
    });

    let filtered_count = original_count - orders.len();
    if filtered_count > 0 {
        println!("\n⚠️  Filtered out {} orders with value < {:.2} USDT", filtered_count, MIN_ORDER_VALUE);
    }

    // 如果所有订单都被过滤了，返回
    if orders.is_empty() {
        println!("\n❌ All orders are below minimum value of {:.2} USDT. No orders to place.", MIN_ORDER_VALUE);
        println!("💡 Try increasing total_buy_value and total_sell_value in config.");
        return Ok(());
    }

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║              GRID ORDERS PREVIEW                       ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║  Total Orders: {:<40} ║", orders.len());
    println!("╚════════════════════════════════════════════════════════╝");

    println!("\n{:<6} {:<15} {:<15} {:<12}", "Side", "Price", "Quantity", "Value");
    println!("{}", "═".repeat(60));

    let mut buy_count = 0;
    let mut sell_count = 0;

    for order in &orders {
        let value = order.price * order.quantity;
        let icon = if order.side == "BUY" { "🟢" } else { "🔴" };

        if order.side == "BUY" {
            buy_count += 1;
        } else {
            sell_count += 1;
        }

        println!(
            "{} {:<4} {:>15.6} {:>15.5} {:>12.2}",
            icon, order.side, order.price, order.quantity, value
        );
    }

    let total_buy_value: f64 = orders
        .iter()
        .filter(|o| o.side == "BUY")
        .map(|o| o.price * o.quantity)
        .sum();

    let total_sell_quantity: f64 = orders
        .iter()
        .filter(|o| o.side == "SELL")
        .map(|o| o.quantity)
        .sum();

    let total_sell_value: f64 = orders
        .iter()
        .filter(|o| o.side == "SELL")
        .map(|o| o.price * o.quantity)
        .sum();

    println!("{}", "═".repeat(50));
    println!("\n📊 ORDER SUMMARY:");
    println!("  🟢 BUY  Orders: {}  |  Total Value: {:.2} USDT", buy_count, total_buy_value);
    println!("  🔴 SELL Orders: {}  |  Total Quantity: {:.5}  |  Total Value: {:.2} USDT",
             sell_count, total_sell_quantity, total_sell_value);
    println!("{}", "═".repeat(50));

    // 确认步骤（仅在手动模式下）
    if !auto_mode {
        println!("\n⚠️  CONFIRMATION REQUIRED");
        println!("{}", "═".repeat(50));
        println!("This will place {} orders on MEXC exchange.", orders.len());
        println!("Please review the orders above carefully.");
        println!("{}", "═".repeat(50));
        println!("\n❓ Do you want to proceed with placing these orders?");
        println!("   Type 'yes' to confirm, or anything else to cancel: ");
        print!("> ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "yes" {
            println!("\n❌ Orders cancelled. No orders were placed.");
            return Ok(());
        }
    }

    println!("\n✅ {} Starting to place orders...", if auto_mode { "Auto mode:" } else { "Confirmed!" });
    println!("{}", "═".repeat(50));

    let mut successful = 0;
    let mut failed = 0;
    let total_orders = orders.len();

    // Convert to BatchOrder format
    let batch_orders: Vec<exchange::BatchOrder> = orders
        .iter()
        .map(|order| exchange::BatchOrder {
            symbol: config.grid.symbol.clone(),
            side: order.side.clone(),
            quantity: order.quantity,
            price: order.price,
        })
        .collect();

    println!("📦 Using batch order API to place {} orders...", total_orders);

    // Use batch API
    match client.place_batch_limit_orders(batch_orders).await {
        Ok(results) => {
            println!("\n{}", "─".repeat(50));
            for (index, result) in results.iter().enumerate() {
                let order = &orders[index];
                let icon = if order.side == "BUY" { "🟢" } else { "🔴" };

                match result {
                    Ok(response) => {
                        println!("{} [{}/{}] {} @ {:.6} ✅ Order ID: {}",
                                 icon, index + 1, total_orders,
                                 order.side, order.price, response.order_id);
                        successful += 1;
                    }
                    Err(e) => {
                        println!("{} [{}/{}] {} @ {:.6} ❌ Failed: {}",
                                 icon, index + 1, total_orders,
                                 order.side, order.price, e);
                        failed += 1;
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Batch order request failed: {}", e);
            failed = total_orders;
        }
    }

    println!("\n{}", "═".repeat(50));
    println!("║           ORDER PLACEMENT COMPLETE                     ║");
    println!("{}", "═".repeat(50));
    println!("  ✅ Successful: {}/{}", successful, total_orders);
    println!("  ❌ Failed:     {}/{}", failed, total_orders);
    println!("{}", "═".repeat(50));

    if failed == 0 {
        println!("\n🎉 All orders placed successfully!");
    } else if successful > 0 {
        println!("\n⚠️  Some orders failed. Please check the errors above.");
    } else {
        println!("\n❌ All orders failed. Please check your configuration and account balance.");
    }

    // 保存当前价格状态
    if successful > 0 {
        let new_state = TradingState::new(config.grid.symbol.clone(), current_price);
        if let Err(e) = new_state.save(&state_file) {
            println!("\n⚠️  Warning: Failed to save state: {}", e);
        } else {
            println!("\n💾 Saved current price state: {:.6}", current_price);
        }
    }

    Ok(())
}

async fn show_open_orders(config_path: &str, all: bool, closed: u32) -> Result<()> {
    let config = Config::from_file(config_path)?;

    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", config.exchange))
        .context(format!("Invalid exchange type: {}", config.exchange))?;

    let client = create_exchange(
        &exchange_type,
        config.api_key.clone(),
        config.api_secret.clone(),
        config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &config.grid.symbol).await?;

    let symbol = if all { None } else { Some(config.grid.symbol.as_str()) };

    println!("Fetching open orders...");
    let orders = client.get_open_orders(symbol).await?;

    if orders.is_empty() {
        println!("No open orders found.");
    } else {
        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║                  OPEN ORDERS                           ║");
        println!("╚════════════════════════════════════════════════════════╝");
        println!("\nOpen Orders ({}):", orders.len());
        println!("{:<15} {:<8} {:<15} {:<15} {:<15} {:<10}",
                 "Symbol", "Side", "Price", "Quantity", "Filled", "Status");
        println!("{}", "-".repeat(85));

        let mut total_value = 0.0;

        for order in orders {
            let price: f64 = order.price.parse().unwrap_or(0.0);
            let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
            let filled: f64 = order.executed_qty.parse().unwrap_or(0.0);

            total_value += price * (qty - filled);

            println!(
                "{:<15} {:<8} {:>15.6} {:>15.5} {:>15.5} {:<10}",
                order.symbol, order.side, price, qty, filled, order.status
            );
        }

        println!("{}", "-".repeat(85));
        println!("Total unfilled value: {:.2} USDT", total_value);
    }

    // 如果请求显示关闭的订单（closed > 0）
    if closed > 0 {
        let limit = closed;
        println!("\n\nFetching recently closed orders...");
        let all_orders = client.get_all_orders(&config.grid.symbol, Some(limit + 100)).await?;

        // 筛选出已关闭的订单
        let closed_orders: Vec<_> = all_orders
            .into_iter()
            .filter(|o| o.status != "NEW" && o.status != "PARTIALLY_FILLED")
            .take(limit as usize)
            .collect();

        if closed_orders.is_empty() {
            println!("No closed orders found.");
        } else {
            println!("\n╔════════════════════════════════════════════════════════╗");
            println!("║              RECENTLY CLOSED ORDERS                    ║");
            println!("╚════════════════════════════════════════════════════════╝");
            println!("\nRecently Closed Orders ({}):", closed_orders.len());
            println!("{:<15} {:<8} {:<15} {:<15} {:<15} {:<15}",
                     "Symbol", "Side", "Price", "Quantity", "Filled", "Status");
            println!("{}", "-".repeat(95));

            for order in closed_orders {
                let price: f64 = order.price.parse().unwrap_or(0.0);
                let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
                let filled: f64 = order.executed_qty.parse().unwrap_or(0.0);

                let status_icon = match order.status.as_str() {
                    "FILLED" => "✅",
                    "CANCELED" => "❌",
                    "EXPIRED" => "⏰",
                    "REJECTED" => "🚫",
                    _ => "❓",
                };

                println!(
                    "{:<15} {:<8} {:>15.6} {:>15.5} {:>15.5} {} {:<12}",
                    order.symbol, order.side, price, qty, filled, status_icon, order.status
                );
            }

            println!("{}", "-".repeat(95));
        }
    }

    Ok(())
}

async fn cancel_all_orders(config_path: &str, force: bool) -> Result<()> {
    let config = Config::from_file(config_path)?;

    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", config.exchange))
        .context(format!("Invalid exchange type: {}", config.exchange))?;

    let client = create_exchange(
        &exchange_type,
        config.api_key.clone(),
        config.api_secret.clone(),
        config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &config.grid.symbol).await?;

    println!("Fetching open orders...");
    let orders = client.get_open_orders(Some(&config.grid.symbol)).await?;

    if orders.is_empty() {
        println!("✅ No open orders to cancel.");
        return Ok(());
    }

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║            ORDERS TO BE CANCELED                       ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!("\nFound {} open orders:", orders.len());
    println!("{:<15} {:<8} {:<15} {:<15}",
             "Symbol", "Side", "Price", "Quantity");
    println!("{}", "-".repeat(60));

    for order in &orders {
        let price: f64 = order.price.parse().unwrap_or(0.0);
        let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);

        println!(
            "{:<15} {:<8} {:>15.6} {:>15.5}",
            order.symbol, order.side, price, qty
        );
    }

    println!("{}", "-".repeat(60));

    // 如果不是强制模式，需要确认
    if !force {
        println!("\n⚠️  WARNING: This will cancel ALL {} orders!", orders.len());
        println!("{}", "═".repeat(60));
        println!("This action CANNOT be undone.");
        println!("{}", "═".repeat(60));
        println!("\n❓ Are you sure you want to cancel all orders?");
        println!("   Type 'yes' to confirm, or anything else to abort: ");
        print!("> ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "yes" {
            println!("\n✅ Canceled. No orders were removed.");
            return Ok(());
        }
    }

    println!("\n🗑️  Canceling all orders...");
    println!("{}", "═".repeat(60));

    let mut successful = 0;
    let mut failed = 0;
    let total_orders = orders.len();

    for (index, order) in orders.iter().enumerate() {
        let price: f64 = order.price.parse().unwrap_or(0.0);
        print!("[{}/{}] Canceling {} order at {:.6}... ",
               index + 1, total_orders, order.side, price);
        std::io::stdout().flush()?;

        match client.cancel_order(&order.symbol, &order.order_id).await {
            Ok(_) => {
                println!("✅ Canceled");
                successful += 1;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            Err(e) => {
                println!("❌ Failed: {}", e);
                failed += 1;
            }
        }
    }

    println!("\n{}", "═".repeat(60));
    println!("║         CANCELLATION COMPLETE                          ║");
    println!("{}", "═".repeat(60));
    println!("  ✅ Canceled:   {}/{}", successful, total_orders);
    println!("  ❌ Failed:     {}/{}", failed, total_orders);
    println!("{}", "═".repeat(60));

    if failed == 0 {
        println!("\n🎉 All orders canceled successfully!");
    } else if successful > 0 {
        println!("\n⚠️  Some orders failed to cancel. Please check the errors above.");
    } else {
        println!("\n❌ All cancellations failed. Please check your connection and try again.");
    }

    Ok(())
}

async fn show_trades(config_path: &str, limit: Option<u32>) -> Result<()> {
    let config = Config::from_file(config_path)?;

    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", config.exchange))
        .context(format!("Invalid exchange type: {}", config.exchange))?;

    let client = create_exchange(
        &exchange_type,
        config.api_key.clone(),
        config.api_secret.clone(),
        config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &config.grid.symbol).await?;

    println!("Fetching trades for {}...", config.grid.symbol);
    let trades = client.get_my_trades(&config.grid.symbol, limit).await?;

    if trades.is_empty() {
        println!("No trades found.");
        return Ok(());
    }

    println!("\nTrade History ({}):", trades.len());
    println!("{:<20} {:<8} {:<15} {:<15} {:<12} {:<12}",
             "Trade ID", "Side", "Price", "Quantity", "Total", "Fee");
    println!("{}", "-".repeat(90));

    let mut total_buy_value = 0.0;
    let mut total_sell_value = 0.0;
    let mut total_fees = 0.0;

    for trade in trades {
        let price: f64 = trade.price.parse().unwrap_or(0.0);
        let qty: f64 = trade.qty.parse().unwrap_or(0.0);
        let total: f64 = trade.quote_qty.parse().unwrap_or(0.0);
        let fee: f64 = trade.commission.parse().unwrap_or(0.0);

        let side = if trade.is_buyer { "BUY" } else { "SELL" };

        if trade.is_buyer {
            total_buy_value += total;
        } else {
            total_sell_value += total;
        }
        total_fees += fee;

        println!(
            "{:<20} {:<8} {:>15.6} {:>15.5} {:>12.2} {:>12.5} {}",
            trade.id, side, price, qty, total, fee, trade.commission_asset
        );
    }

    println!("{}", "-".repeat(90));
    println!("Total BUY value:  {:.2} USDT", total_buy_value);
    println!("Total SELL value: {:.2} USDT", total_sell_value);
    println!("Total fees:       {:.5}", total_fees);

    if total_sell_value > 0.0 && total_buy_value > 0.0 {
        let profit = total_sell_value - total_buy_value;
        println!("Net profit/loss:  {:.2} USDT ({:.2}%)",
                 profit, (profit / total_buy_value) * 100.0);
    }

    Ok(())
}
