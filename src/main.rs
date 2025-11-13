mod account_snapshot;
mod api_server;
mod config;
mod exchange;
mod exchanges;
mod order_calculator;
mod price_calculator;
mod state;

use account_snapshot::{AccountSnapshot, AssetSnapshot, PnLAnalyzer, SnapshotHistory};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use exchange::Exchange;
use exchanges::{create_exchange, ExchangeType};
use order_calculator::OrderCalculator;
use price_calculator::PriceCalculator;
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
    /// 启动 REST API 服务器提供实时报告
    Report {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
        /// API 服务器端口
        #[arg(short, long, default_value = "3000")]
        port: u16,
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
        Commands::Orders { config, closed } => {
            show_open_orders(&config, closed).await?;
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
        Commands::Report { config, port } => {
            api_server::start_server(config, port).await?;
        }
    }

    Ok(())
}

/// 捕获账户快照（包含账户信息和当前价格）
async fn capture_account_snapshot(
    client: &Arc<dyn Exchange>,
    exchange: &str,
    symbol: &str,
    mid_price: Option<f64>,
    iteration: Option<u64>,
) -> Result<AccountSnapshot> {
    let account = client.get_account_info().await?;
    let (base_asset, quote_asset) = client.get_symbol_assets(symbol);

    let mut assets = Vec::new();
    for balance in &account.balances {
        if balance.asset == base_asset || balance.asset == quote_asset {
            let free: f64 = balance.free.parse().unwrap_or(0.0);
            let locked: f64 = balance.locked.parse().unwrap_or(0.0);
            let total = free + locked;

            assets.push(AssetSnapshot {
                asset: balance.asset.clone(),
                free,
                locked,
                total,
            });
        }
    }

    Ok(AccountSnapshot::new(exchange, symbol, assets, mid_price, iteration))
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

async fn show_market_depth(client: &Arc<dyn Exchange>, symbol: &str) -> Result<()> {
    // 获取订单簿（50档以获取足够深度）
    let order_book = client.get_order_book(symbol, Some(50)).await?;

    if order_book.bids.is_empty() || order_book.asks.is_empty() {
        anyhow::bail!("Order book is empty");
    }

    // 获取最优买卖价
    let best_bid: f64 = order_book.bids[0][0].parse().context("Failed to parse best bid")?;
    let best_ask: f64 = order_book.asks[0][0].parse().context("Failed to parse best ask")?;
    let mid_price = (best_bid + best_ask) / 2.0;

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║                  MARKET DEPTH                          ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!("  Current Price: {:.6} (Bid: {:.6} | Ask: {:.6})", mid_price, best_bid, best_ask);
    println!();

    // 定义要统计的百分比范围
    let percentages = vec![0.5, 1.0, 2.0, 5.0, 10.0];

    println!("  {:<12} {:<20} {:<20}", "Range", "Bid Depth (USDT)", "Ask Depth (USDT)");
    println!("  {}", "─".repeat(54));

    for pct in percentages {
        let bid_depth = calculate_depth_in_range(&order_book.bids, mid_price, pct, true)?;
        let ask_depth = calculate_depth_in_range(&order_book.asks, mid_price, pct, false)?;

        println!("  ±{:<10.1}% {:>18.2} {:>20.2}", pct, bid_depth, ask_depth);
    }

    println!("  {}", "─".repeat(54));
    println!();

    Ok(())
}

/// 显示订单深度统计（只显示深度，不显示具体订单列表）
async fn show_order_depth_summary(client: &Arc<dyn Exchange>, symbol: &str) -> Result<()> {
    let orders = client.get_open_orders(Some(symbol)).await?;

    if orders.is_empty() {
        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║              MY ORDER DEPTH (NONE)                     ║");
        println!("╚════════════════════════════════════════════════════════╝");
        println!("  No open orders found.\n");
        return Ok(());
    }

    // 统计买单和卖单
    let buy_orders: Vec<_> = orders.iter().filter(|o| o.side == "BUY").collect();
    let sell_orders: Vec<_> = orders.iter().filter(|o| o.side == "SELL").collect();

    let mut total_buy_value = 0.0;
    let mut total_buy_qty = 0.0;
    let mut total_sell_value = 0.0;
    let mut total_sell_qty = 0.0;

    let mut buy_prices = Vec::new();
    let mut sell_prices = Vec::new();

    for order in &buy_orders {
        let price: f64 = order.price.parse().unwrap_or(0.0);
        let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
        let filled: f64 = order.executed_qty.parse().unwrap_or(0.0);
        let remaining_qty = qty - filled;

        total_buy_value += price * remaining_qty;
        total_buy_qty += remaining_qty;
        buy_prices.push(price);
    }

    for order in &sell_orders {
        let price: f64 = order.price.parse().unwrap_or(0.0);
        let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
        let filled: f64 = order.executed_qty.parse().unwrap_or(0.0);
        let remaining_qty = qty - filled;

        total_sell_value += price * remaining_qty;
        total_sell_qty += remaining_qty;
        sell_prices.push(price);
    }

    // 计算价格范围
    let (buy_min, buy_max) = if !buy_prices.is_empty() {
        let min = buy_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = buy_prices.iter().fold(0.0f64, |a, &b| a.max(b));
        (Some(min), Some(max))
    } else {
        (None, None)
    };

    let (sell_min, sell_max) = if !sell_prices.is_empty() {
        let min = sell_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = sell_prices.iter().fold(0.0f64, |a, &b| a.max(b));
        (Some(min), Some(max))
    } else {
        (None, None)
    };

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║                MY ORDER DEPTH                          ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!("  Total Orders: {}", orders.len());
    println!();

    println!("  🟢 BUY Orders:");
    println!("     Count:       {}", buy_orders.len());
    if let (Some(min), Some(max)) = (buy_min, buy_max) {
        println!("     Price Range: {:.6} - {:.6}", min, max);
    }
    println!("     Total Qty:   {:.5}", total_buy_qty);
    println!("     Total Value: {:.2} USDT", total_buy_value);
    println!();

    println!("  🔴 SELL Orders:");
    println!("     Count:       {}", sell_orders.len());
    if let (Some(min), Some(max)) = (sell_min, sell_max) {
        println!("     Price Range: {:.6} - {:.6}", min, max);
    }
    println!("     Total Qty:   {:.5}", total_sell_qty);
    println!("     Total Value: {:.2} USDT", total_sell_value);
    println!();

    println!("  💰 Total Unfilled Value: {:.2} USDT", total_buy_value + total_sell_value);
    println!();

    Ok(())
}

/// 计算指定百分比范围内的订单簿深度（USDT）
fn calculate_depth_in_range(
    orders: &[Vec<String>],
    mid_price: f64,
    percentage: f64,
    is_bid: bool,
) -> Result<f64> {
    let mut total_depth = 0.0;

    // 计算价格范围
    let price_threshold = if is_bid {
        // 买单：从mid_price向下percentage%
        mid_price * (1.0 - percentage / 100.0)
    } else {
        // 卖单：从mid_price向上percentage%
        mid_price * (1.0 + percentage / 100.0)
    };

    for order in orders {
        if order.len() < 2 {
            continue;
        }

        let price: f64 = order[0].parse().context("Failed to parse price")?;
        let quantity: f64 = order[1].parse().context("Failed to parse quantity")?;

        // 判断是否在范围内
        let in_range = if is_bid {
            price >= price_threshold && price <= mid_price
        } else {
            price <= price_threshold && price >= mid_price
        };

        if in_range {
            total_depth += price * quantity;
        }
    }

    Ok(total_depth)
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
        match place_orders_internal_with_iteration(config_path, true, Some(iteration)).await {
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
    place_orders_internal_with_iteration(config_path, auto_mode, None).await
}

async fn place_orders_internal_with_iteration(
    config_path: &str,
    auto_mode: bool,
    iteration: Option<u64>,
) -> Result<()> {
    println!("Loading config from: {}", config_path);
    let config = Config::from_file(config_path)?;

    println!("\n{}", "═".repeat(70));
    println!("  Configured exchanges: {}", config.exchanges.len());
    for (i, exchange) in config.exchanges.iter().enumerate() {
        println!("    {}. {}", i + 1, exchange.name);
    }
    println!("{}", "═".repeat(70));

    let mut all_success = true;

    for (i, exchange_config) in config.exchanges.iter().enumerate() {
        println!("\n\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║  Processing Exchange {}/{}: {:<37} ║", i + 1, config.exchanges.len(), exchange_config.name);
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        match place_orders_for_exchange(exchange_config, &config.grid, auto_mode, iteration).await {
            Ok(_) => println!("\n✅ Successfully processed {}", exchange_config.name),
            Err(e) => {
                println!("\n❌ Failed to process {}: {}", exchange_config.name, e);
                all_success = false;
            }
        }
    }

    if all_success {
        println!("\n\n🎉 All exchanges processed successfully!");
    } else {
        println!("\n\n⚠️  Some exchanges failed. Please check the errors above.");
    }

    Ok(())
}

async fn place_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
    auto_mode: bool,
    iteration: Option<u64>,
) -> Result<()> {
    // 解析交易所类型
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    println!("Exchange: {:?}", exchange_type);

    // 创建对应的交易所客户端
    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &grid_config.symbol).await?;

    println!("Fetching order book for {}...", grid_config.symbol);

    // 获取订单簿（根据配置的深度）
    let order_book = client
        .get_order_book(&grid_config.symbol, Some(grid_config.orderbook_depth))
        .await?;

    if order_book.bids.is_empty() || order_book.asks.is_empty() {
        anyhow::bail!("Order book is empty, cannot calculate mid price");
    }

    // 如果 Bid 或 Ask 使用 VolumeThreshold 方法，需要获取历史订单
    let all_orders = if matches!(grid_config.mid_price_method_bid, config::MidPriceMethod::VolumeThreshold)
        || matches!(grid_config.mid_price_method_ask, config::MidPriceMethod::VolumeThreshold)
    {
        println!("Fetching order history to calculate filled buy orders value...");
        match client.get_all_orders(&grid_config.symbol, Some(500)).await {
            Ok(orders) => Some(orders),
            Err(e) => {
                println!("⚠️  Warning: Failed to fetch order history: {}. Using default threshold.", e);
                None
            }
        }
    } else {
        None
    };

    // 使用 PriceCalculator 分别计算 bid 和 ask 价格
    let price_result = PriceCalculator::calculate_mid_price(
        &order_book,
        &grid_config.mid_price_method_bid,
        &grid_config.mid_price_method_ask,
        all_orders.as_deref(),
        grid_config.volume_threshold_usdt,
    )?;

    let current_price = price_result.mid_price;

    println!("\n📊 Price Calculation:");
    println!("  Methods: {}", price_result.method_description);
    println!("{}", price_result.details);
    println!("  => Mid Price: {:.6}", current_price);

    // 读取上次布单的状态（为每个交易所单独保存状态）
    let state_file = format!(".state_{}_{}.json", exchange_config.name, grid_config.symbol);
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

            let all_orders = client.get_open_orders(Some(&grid_config.symbol)).await?;
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
    let mut orders = OrderCalculator::calculate_grid_orders(current_price, grid_config);

    // 获取现有挂单
    println!("\n🔍 Checking existing orders...");
    let existing_orders = client.get_open_orders(Some(&grid_config.symbol)).await?;

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
            let mut adjusted_config = grid_config.clone();
            let remaining_buy_value = (grid_config.total_buy_value - existing_buy_value).max(0.0);
            let remaining_sell_value = (grid_config.total_sell_value - existing_sell_value).max(0.0);

            // 如果调整后的价值小于最小订单价值，设置为 0（不布单）
            if remaining_buy_value < grid_config.minimal_order_value {
                adjusted_config.total_buy_value = 0.0;
                println!("  Adjusted buy value {:.2} USDT < minimal {:.2} USDT, skipping buy orders",
                         remaining_buy_value, grid_config.minimal_order_value);
            } else {
                adjusted_config.total_buy_value = remaining_buy_value;
                println!("  Adjusted buy value for new orders: {:.2} USDT", adjusted_config.total_buy_value);
            }

            if remaining_sell_value < grid_config.minimal_order_value {
                adjusted_config.total_sell_value = 0.0;
                println!("  Adjusted sell value {:.2} USDT < minimal {:.2} USDT, skipping sell orders",
                         remaining_sell_value, grid_config.minimal_order_value);
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
            symbol: grid_config.symbol.clone(),
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
        let new_state = TradingState::new(grid_config.symbol.clone(), current_price);
        if let Err(e) = new_state.save(&state_file) {
            println!("\n⚠️  Warning: Failed to save state: {}", e);
        } else {
            println!("\n💾 Saved current price state: {:.6}", current_price);
        }
    }

    // 记录账户快照（无论布单是否成功都记录）
    println!("\n📸 Capturing account snapshot...");
    match capture_account_snapshot(&client, &exchange_config.name, &grid_config.symbol, Some(current_price), iteration).await {
        Ok(snapshot) => {
            let history = SnapshotHistory::new(&exchange_config.name, &grid_config.symbol);
            match history.append_snapshot(&snapshot) {
                Ok(_) => {
                    println!("✅ Account snapshot saved");

                    // 如果有之前的快照，计算并显示盈亏
                    if let Ok(Some(previous)) = history.load_latest().and_then(|_latest| {
                        // 获取倒数第二个快照
                        let all = history.load_all()?;
                        if all.len() >= 2 {
                            Ok(Some(all[all.len() - 2].clone()))
                        } else {
                            Ok(None)
                        }
                    }) {
                        let (base_asset, quote_asset) = client.get_symbol_assets(&grid_config.symbol);
                        let report = PnLAnalyzer::analyze_change(&previous, &snapshot, &base_asset, &quote_asset);

                        if let Some(vc) = &report.value_change {
                            let icon = if vc.change >= 0.0 { "📈" } else { "📉" };
                            println!("   {} Change since last snapshot: {:+.2} {} ({:+.3}%)",
                                     icon, vc.change, quote_asset, vc.change_percentage);
                        }
                    }
                },
                Err(e) => println!("⚠️  Warning: Failed to save snapshot: {}", e),
            }
        },
        Err(e) => println!("⚠️  Warning: Failed to capture snapshot: {}", e),
    }

    Ok(())
}

async fn show_open_orders(config_path: &str, closed: u32) -> Result<()> {
    let config = Config::from_file(config_path)?;

    println!("\n{}", "═".repeat(70));
    println!("  Configured exchanges: {}", config.exchanges.len());
    for (i, exchange) in config.exchanges.iter().enumerate() {
        println!("    {}. {}", i + 1, exchange.name);
    }
    println!("{}", "═".repeat(70));

    for (i, exchange_config) in config.exchanges.iter().enumerate() {
        println!("\n\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║  Exchange {}/{}: {:<45} ║", i + 1, config.exchanges.len(), exchange_config.name);
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        match show_orders_for_exchange(exchange_config, &config.grid, closed).await {
            Ok(_) => {},
            Err(e) => println!("\n❌ Failed to fetch orders from {}: {}", exchange_config.name, e),
        }
    }

    Ok(())
}

async fn show_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
    closed: u32,
) -> Result<()> {
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &grid_config.symbol).await?;

    // 获取并显示市场深度
    println!("\nFetching market depth...");
    if let Err(e) = show_market_depth(&client, &grid_config.symbol).await {
        println!("⚠️  Warning: Failed to fetch market depth: {}", e);
    }

    // 只查询配置文件中指定的交易对
    println!("\nFetching open orders for {}...", grid_config.symbol);
    let orders = client.get_open_orders(Some(&grid_config.symbol)).await?;

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
        let all_orders = client.get_all_orders(&grid_config.symbol, Some(limit + 100)).await?;

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

    println!("\n{}", "═".repeat(70));
    println!("  Configured exchanges: {}", config.exchanges.len());
    for (i, exchange) in config.exchanges.iter().enumerate() {
        println!("    {}. {}", i + 1, exchange.name);
    }
    println!("{}", "═".repeat(70));

    for (i, exchange_config) in config.exchanges.iter().enumerate() {
        println!("\n\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║  Exchange {}/{}: {:<45} ║", i + 1, config.exchanges.len(), exchange_config.name);
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        match cancel_orders_for_exchange(exchange_config, &config.grid, force).await {
            Ok(_) => {},
            Err(e) => println!("\n❌ Failed to cancel orders on {}: {}", exchange_config.name, e),
        }
    }

    Ok(())
}

async fn cancel_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
    force: bool,
) -> Result<()> {
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &grid_config.symbol).await?;

    println!("Fetching open orders...");
    let orders = client.get_open_orders(Some(&grid_config.symbol)).await?;

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

async fn show_pnl_report(config_path: &str, recent: usize) -> Result<()> {
    println!("🔄 Starting report mode with 30-second refresh");
    println!("⚠️  Press Ctrl+C to stop\n");
    println!("{}", "═".repeat(60));

    loop {
        // 清屏（可选，如果终端支持）
        print!("\x1B[2J\x1B[1;1H");

        let now = chrono::Utc::now();
        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║  Report Time: {:<39} ║", now.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("╚════════════════════════════════════════════════════════╝\n");

        let config = Config::from_file(config_path)?;

        println!("{}", "═".repeat(70));
        println!("  Configured exchanges: {}", config.exchanges.len());
        for (i, exchange) in config.exchanges.iter().enumerate() {
            println!("    {}. {}", i + 1, exchange.name);
        }
        println!("{}", "═".repeat(70));

        for (i, exchange_config) in config.exchanges.iter().enumerate() {
            println!("\n\n╔═══════════════════════════════════════════════════════════════╗");
            println!("║  Exchange {}/{}: {:<45} ║", i + 1, config.exchanges.len(), exchange_config.name);
            println!("╚═══════════════════════════════════════════════════════════════╝\n");

            match show_pnl_report_for_exchange(exchange_config, &config.grid, recent).await {
                Ok(_) => {},
                Err(e) => println!("\n❌ Failed to generate report for {}: {}", exchange_config.name, e),
            }
        }

        println!("\n⏳ Next refresh in 30 seconds...");
        println!("   Next update at: {}", (now + chrono::Duration::seconds(30)).format("%H:%M:%S UTC"));
        println!("{}", "═".repeat(60));

        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    }
}

async fn show_pnl_report_for_exchange(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
    recent: usize,
) -> Result<()> {
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    let (base_asset, quote_asset) = client.get_symbol_assets(&grid_config.symbol);

    // 显示账户信息
    show_account_info(&client, &grid_config.symbol).await?;

    // 显示市场深度
    if let Err(e) = show_market_depth(&client, &grid_config.symbol).await {
        println!("⚠️  Warning: Failed to fetch market depth: {}", e);
    }

    // 显示订单深度统计（不显示具体订单列表）
    if let Err(e) = show_order_depth_summary(&client, &grid_config.symbol).await {
        println!("⚠️  Warning: Failed to fetch order depth: {}", e);
    }

    // 加载快照历史
    let history = SnapshotHistory::new(&exchange_config.name, &grid_config.symbol);
    let snapshots = history.load_all()?;

    if snapshots.is_empty() {
        println!("No snapshots found. Start using watch mode to collect account data.");
        return Ok(());
    }

    println!("📊 Found {} snapshots in history\n", snapshots.len());

    // 显示最新的账户状态
    if let Some(latest) = snapshots.last() {
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║              LATEST ACCOUNT STATUS                     ║");
        println!("╚════════════════════════════════════════════════════════╝");
        println!("\n📅 Timestamp: {}", latest.datetime);
        if let Some(iter) = latest.iteration {
            println!("🔄 Iteration: #{}", iter);
        }
        if let Some(price) = latest.mid_price {
            println!("💹 Mid Price: {:.6}", price);
        }

        println!("\n🪙 Assets:");
        for asset in &latest.assets {
            println!("  {} {:<8}  Free: {:>15.8}  Locked: {:>15.8}  Total: {:>15.8}",
                     if asset.asset == base_asset { "🪙" } else { "💵" },
                     asset.asset,
                     asset.free,
                     asset.locked,
                     asset.total);
        }

        if let Some(total_value) = latest.calculate_total_value(&base_asset, &quote_asset) {
            println!("\n💰 Total Value: {:.2} {}", total_value, quote_asset);
        }

        println!("════════════════════════════════════════════════════════\n");
    }

    // 如果有多个快照，分析整体表现
    if snapshots.len() >= 2 {
        if let Some(analysis) = PnLAnalyzer::analyze_period(&snapshots, &base_asset, &quote_asset) {
            analysis.print_detailed_report(&base_asset, &quote_asset);
        }

        // 显示最近 N 个快照的详细变化
        let display_count = recent.min(snapshots.len());
        let recent_snapshots = &snapshots[snapshots.len() - display_count..];

        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║          RECENT {} SNAPSHOTS DETAILS                    ║", display_count);
        println!("╚════════════════════════════════════════════════════════╝\n");

        for snapshot in recent_snapshots {
            println!("📸 {}", snapshot.datetime);
            if let Some(iter) = snapshot.iteration {
                println!("   Iteration: #{}", iter);
            }
            if let Some(price) = snapshot.mid_price {
                println!("   Mid Price: {:.6}", price);
            }
            if let Some(total_value) = snapshot.calculate_total_value(&base_asset, &quote_asset) {
                println!("   Total Value: {:.2} {}", total_value, quote_asset);
            }
            for asset in &snapshot.assets {
                println!("   {} {:<8} Total: {:>15.8}",
                         if asset.asset == base_asset { "🪙" } else { "💵" },
                         asset.asset,
                         asset.total);
            }
            println!();
        }
    } else {
        println!("Not enough snapshots for analysis. Keep running watch mode to collect more data.");
    }

    Ok(())
}

async fn show_trades(config_path: &str, limit: Option<u32>) -> Result<()> {
    let config = Config::from_file(config_path)?;

    println!("\n{}", "═".repeat(70));
    println!("  Configured exchanges: {}", config.exchanges.len());
    for (i, exchange) in config.exchanges.iter().enumerate() {
        println!("    {}. {}", i + 1, exchange.name);
    }
    println!("{}", "═".repeat(70));

    for (i, exchange_config) in config.exchanges.iter().enumerate() {
        println!("\n\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║  Exchange {}/{}: {:<45} ║", i + 1, config.exchanges.len(), exchange_config.name);
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        match show_trades_for_exchange(exchange_config, &config.grid, limit).await {
            Ok(_) => {},
            Err(e) => println!("\n❌ Failed to fetch trades from {}: {}", exchange_config.name, e),
        }
    }

    Ok(())
}

async fn show_trades_for_exchange(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
    limit: Option<u32>,
) -> Result<()> {
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, &grid_config.symbol).await?;

    println!("Fetching trades for {}...", grid_config.symbol);
    let trades = client.get_my_trades(&grid_config.symbol, limit).await?;

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
