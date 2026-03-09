mod account_snapshot;
mod api_server;
mod config;
mod exchange;
mod exchanges;
mod order_calculator;
mod price_calculator;
mod state;
mod trade_analysis;
mod trade_history;

use account_snapshot::{AccountSnapshot, AssetSnapshot, PnLAnalyzer, SnapshotHistory};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use exchange::{BatchOrder, Exchange, OrderBook};
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
    /// 显示基于交易历史的统计（成交均价等）
    TradeStats {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
        /// 交易所名称
        #[arg(short, long)]
        exchange: String,
        /// 最多显示最近N笔交易的统计（0表示全部）
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// 刷量：通过两个账户对敲制造成交量（需要在config中配置volume字段）
    CreateVolume {
        /// 配置文件路径
        #[arg(default_value = "config.json")]
        config: String,
        /// 选择交易所（不指定则使用配置中的第一个）
        #[arg(short = 'c', long)]
        exchange: Option<String>,
        /// 方向：buy（制造买入成交）或 sell（制造卖出成交）
        #[arg(short, long)]
        direction: String,
        /// 成交金额（USDT），0表示使用config中volume.default_value的值
        #[arg(short, long, default_value_t = 0.0)]
        value: f64,
        /// 重复次数，默认1次
        #[arg(short, long, default_value_t = 1)]
        repeat: u32,
        /// 重复间隔（秒），默认120秒（2分钟）
        #[arg(short, long, default_value_t = 120)]
        interval: u64,
        /// 切换方向次数，默认0（不切换）
        #[arg(short, long, default_value_t = 0)]
        toggle: u32,
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
        Commands::TradeStats { config, exchange, limit } => {
            show_trade_stats(&config, &exchange, limit).await?;
        }
        Commands::CreateVolume { config, exchange, direction, value, repeat, interval, toggle } => {
            create_volume(&config, exchange.as_deref(), &direction, value, repeat, interval, toggle).await?;
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

    // Step 1: Collect mid prices from all exchanges
    println!("\n📊 Step 1: Collecting mid prices from all exchanges...");
    let mut mid_prices = Vec::new();

    for exchange_config in &config.exchanges {
        match get_exchange_mid_price(exchange_config, &config).await {
            Ok(price) => {
                println!("  ✅ {}: {:.6}", exchange_config.name, price);
                mid_prices.push(price);
            }
            Err(e) => {
                println!("  ⚠️  {}: Failed to get mid price - {}", exchange_config.name, e);
            }
        }
    }

    if mid_prices.is_empty() {
        anyhow::bail!("Failed to get mid price from any exchange");
    }

    // Step 2: Calculate unified mid price (average)
    let unified_mid_price = mid_prices.iter().sum::<f64>() / mid_prices.len() as f64;

    println!("\n💰 Step 2: Calculating unified mid price");
    println!("  Individual mid prices: {:?}", mid_prices.iter().map(|p| format!("{:.6}", p)).collect::<Vec<_>>());
    println!("  Unified mid price (average): {:.6}", unified_mid_price);
    println!("  🔒 All exchanges will use this unified price to prevent arbitrage");

    // Step 3: Calculate grid orders once using unified mid price
    println!("\n📝 Step 3: Calculating grid orders using unified mid price...");
    use crate::order_calculator::OrderCalculator;

    // Select the appropriate grid config based on unified mid price
    let selected_grid_config = config.grid.select_config(unified_mid_price);
    let unified_orders = OrderCalculator::calculate_grid_orders(unified_mid_price, &selected_grid_config);

    println!("  Buy orders: {}", unified_orders.iter().filter(|o| o.side == "BUY").count());
    println!("  Sell orders: {}", unified_orders.iter().filter(|o| o.side == "SELL").count());
    if let Some(first_buy) = unified_orders.iter().find(|o| o.side == "BUY") {
        println!("  First buy order: {:.8} USDT", first_buy.price);
    }
    if let Some(first_sell) = unified_orders.iter().find(|o| o.side == "SELL") {
        println!("  First sell order: {:.8} USDT", first_sell.price);
    }

    // Step 4: Place unified orders on all exchanges
    println!("\n📝 Step 4: Placing unified orders on all exchanges...");
    let mut all_success = true;

    for (i, exchange_config) in config.exchanges.iter().enumerate() {
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║  Processing Exchange {}/{}: {:<37} ║", i + 1, config.exchanges.len(), exchange_config.name);
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        match place_unified_orders_for_exchange(
            exchange_config,
            config.grid.get_symbol(),
            &unified_orders,
            auto_mode,
            iteration
        ).await {
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

/// Get mid price from a single exchange
async fn get_exchange_mid_price(
    exchange_config: &config::ExchangeConfig,
    config: &config::Config,
) -> Result<f64> {
    

    let symbol = config.grid.get_symbol();

    // Parse exchange type
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    // Create exchange client
    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // Get order book
    let order_book = client
        .get_order_book(symbol, Some(config.orderbook_depth))
        .await?;

    if order_book.bids.is_empty() || order_book.asks.is_empty() {
        anyhow::bail!("Order book is empty");
    }

    // Get all orders if needed for VolumeThreshold method
    let all_orders = if matches!(config.mid_price_method_bid, config::MidPriceMethod::VolumeThreshold)
        || matches!(config.mid_price_method_ask, config::MidPriceMethod::VolumeThreshold)
    {
        match client.get_all_orders(symbol, Some(500)).await {
            Ok(orders) => Some(orders),
            Err(_) => None,
        }
    } else {
        None
    };

    // Calculate mid price for this exchange
    let price_result = PriceCalculator::calculate_mid_price(
        &order_book,
        &config.mid_price_method_bid,
        &config.mid_price_method_ask,
        all_orders.as_deref(),
        config.volume_threshold_usdt,
        config.fix_mid_price,
    )?;

    Ok(price_result.mid_price)
}

/// Place unified orders on a specific exchange
async fn place_unified_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    symbol: &str,
    unified_orders: &[order_calculator::GridOrder],
    auto_mode: bool,
    iteration: Option<u64>,
) -> Result<()> {
    // Parse exchange type
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    println!("Exchange: {:?}", exchange_type);

    // Create exchange client
    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // Show account info
    show_account_info(&client, symbol).await?;

    println!("\n📋 Using unified orders (averaged across all exchanges):");
    println!("  Buy orders: {}", unified_orders.iter().filter(|o| o.side == "BUY").count());
    println!("  Sell orders: {}", unified_orders.iter().filter(|o| o.side == "SELL").count());

    // Show some example prices
    if let Some(first_buy) = unified_orders.iter().find(|o| o.side == "BUY") {
        println!("  First buy order: {:.8} USDT", first_buy.price);
    }
    if let Some(first_sell) = unified_orders.iter().find(|o| o.side == "SELL") {
        println!("  First sell order: {:.8} USDT", first_sell.price);
    }

    // Get existing open orders
    let open_orders = client.get_open_orders(Some(symbol)).await?;
    println!("\n📊 Current open orders: {}", open_orders.len());

    // Cancel existing orders
    if !open_orders.is_empty() {
        println!("🔄 Canceling {} existing orders...", open_orders.len());
        for order in &open_orders {
            match client.cancel_order(symbol, &order.order_id).await {
                Ok(_) => {},
                Err(e) => println!("  ⚠️  Warning: Failed to cancel order {}: {}", order.order_id, e),
            }
        }
        println!("  ✅ Canceled existing orders");
    }

    // Save snapshot
    if let Some(iter) = iteration {
        // Calculate reference price from unified orders
        let buy_prices: Vec<f64> = unified_orders.iter()
            .filter(|o| o.side == "BUY")
            .map(|o| o.price)
            .collect();
        let sell_prices: Vec<f64> = unified_orders.iter()
            .filter(|o| o.side == "SELL")
            .map(|o| o.price)
            .collect();

        let reference_price = if !buy_prices.is_empty() && !sell_prices.is_empty() {
            (buy_prices.iter().sum::<f64>() / buy_prices.len() as f64 + sell_prices.iter().sum::<f64>() / sell_prices.len() as f64) / 2.0
        } else if !buy_prices.is_empty() {
            buy_prices.iter().sum::<f64>() / buy_prices.len() as f64
        } else if !sell_prices.is_empty() {
            sell_prices.iter().sum::<f64>() / sell_prices.len() as f64
        } else {
            0.0
        };

        if reference_price > 0.0 {
            match capture_account_snapshot(&client, &exchange_config.name, symbol, Some(reference_price), Some(iter)).await {
                Ok(snapshot) => {
                    let history = SnapshotHistory::new(&exchange_config.name, symbol);
                    if let Err(e) = history.append_snapshot(&snapshot) {
                        println!("⚠️  Warning: Failed to save snapshot: {}", e);
                    }
                }
                Err(e) => println!("⚠️  Warning: Failed to capture snapshot: {}", e),
            }
        }
    }

    // Confirm before placing orders
    if !auto_mode {
        print!("\nPress Enter to continue with placing {} orders, or Ctrl+C to cancel: ", unified_orders.len());
        std::io::stdin().read_line(&mut String::new())?;
    }

    // Check account balance before placing orders
    println!("\n💰 Checking account balance...");
    let account = client.get_account_info().await?;
    let (base_asset, quote_asset) = client.get_symbol_assets(symbol);

    let mut usdt_available = 0.0;
    let mut base_available = 0.0;
    let mut usdt_locked = 0.0;
    let mut base_locked = 0.0;

    for balance in &account.balances {
        if balance.asset == quote_asset {
            usdt_available = balance.free.parse().unwrap_or(0.0);
            usdt_locked = balance.locked.parse().unwrap_or(0.0);
        } else if balance.asset == base_asset {
            base_available = balance.free.parse().unwrap_or(0.0);
            base_locked = balance.locked.parse().unwrap_or(0.0);
        }
    }

    println!("  Available {} balance: {:.2} (locked: {:.2})", quote_asset, usdt_available, usdt_locked);
    println!("  Available {} balance: {:.8} (locked: {:.8})", base_asset, base_available, base_locked);

    // Calculate required resources
    let total_buy_value: f64 = unified_orders.iter()
        .filter(|o| o.side == "BUY")
        .map(|o| o.price * o.quantity)
        .sum();
    let total_sell_quantity: f64 = unified_orders.iter()
        .filter(|o| o.side == "SELL")
        .map(|o| o.quantity)
        .sum();

    println!("  Required for planned orders:");
    println!("    Buy orders need: {:.2} {}", total_buy_value, quote_asset);
    println!("    Sell orders need: {:.8} {}", total_sell_quantity, base_asset);

    // Sort orders: buy orders by price (high to low), sell orders by price (low to high)
    let mut sorted_orders: Vec<_> = unified_orders.iter().cloned().collect();
    sorted_orders.sort_by(|a, b| {
        if a.side == "BUY" && b.side == "BUY" {
            b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal)
        } else if a.side == "SELL" && b.side == "SELL" {
            a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal)
        } else if a.side == "BUY" {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    // Filter orders based on available balance
    let mut filtered_orders = Vec::new();
    let mut skipped_buy_count = 0;
    let mut skipped_sell_count = 0;
    let mut required_usdt = 0.0;
    let mut required_base = 0.0;

    for order in sorted_orders {
        if order.side == "BUY" {
            let order_value = order.price * order.quantity;
            if required_usdt + order_value <= usdt_available {
                required_usdt += order_value;
                filtered_orders.push(order);
            } else {
                skipped_buy_count += 1;
                println!("  ⚠️  Skipping BUY order @ {:.6} (value: {:.2} USDT) - insufficient balance",
                         order.price, order_value);
            }
        } else if order.side == "SELL" {
            if required_base + order.quantity <= base_available {
                required_base += order.quantity;
                filtered_orders.push(order);
            } else {
                skipped_sell_count += 1;
                println!("  ⚠️  Skipping SELL order @ {:.6} (qty: {:.8} {}) - insufficient balance",
                         order.price, order.quantity, base_asset);
            }
        }
    }

    if skipped_buy_count > 0 || skipped_sell_count > 0 {
        println!("\n⚠️  Skipped {} BUY and {} SELL orders due to insufficient balance",
                 skipped_buy_count, skipped_sell_count);
    }

    if filtered_orders.is_empty() {
        println!("\n❌ All orders skipped due to insufficient balance. No orders to place.");
        return Ok(());
    }

    let unified_orders = filtered_orders;

    // Place unified orders using batch API
    println!("\n📝 Placing {} unified orders using batch API...", unified_orders.len());

    // Convert GridOrder to BatchOrder
    let batch_orders: Vec<crate::exchange::BatchOrder> = unified_orders.iter()
        .map(|order| crate::exchange::BatchOrder {
            symbol: symbol.to_string(),
            side: order.side.clone(),
            quantity: order.quantity,
            price: order.price,
        })
        .collect();

    // Use batch order placement
    let results = client.place_batch_limit_orders(batch_orders).await?;

    // Count successes and failures
    let mut success_count = 0;
    let mut fail_count = 0;

    for (i, result) in results.iter().enumerate() {
        let order = &unified_orders[i];
        print!("  [{}/{}] {} {} @ {:.8} (qty: {:.5}) ... ",
            i + 1, unified_orders.len(),
            order.side, symbol,
            order.price, order.quantity
        );

        match result {
            Ok(response) => {
                println!("✅ Order ID: {}", response.order_id);
                success_count += 1;
            }
            Err(e) => {
                println!("❌ Failed: {}", e);
                fail_count += 1;
            }
        }
    }

    println!("\n📊 Order placement summary:");
    println!("  ✅ Successful: {}", success_count);
    println!("  ❌ Failed: {}", fail_count);

    Ok(())
}

async fn place_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    config: &config::Config,
    auto_mode: bool,
    iteration: Option<u64>,
) -> Result<()> {
    let symbol = config.grid.get_symbol();

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
    show_account_info(&client, symbol).await?;

    println!("Fetching order book for {}...", symbol);

    // 获取订单簿（根据配置的深度）
    let order_book = client
        .get_order_book(symbol, Some(config.orderbook_depth))
        .await?;

    if order_book.bids.is_empty() || order_book.asks.is_empty() {
        anyhow::bail!("Order book is empty, cannot calculate mid price");
    }

    // 如果 Bid 或 Ask 使用 VolumeThreshold 方法，需要获取历史订单
    let all_orders = if matches!(config.mid_price_method_bid, config::MidPriceMethod::VolumeThreshold)
        || matches!(config.mid_price_method_ask, config::MidPriceMethod::VolumeThreshold)
    {
        println!("Fetching order history to calculate filled buy orders value...");
        match client.get_all_orders(symbol, Some(500)).await {
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
        &config.mid_price_method_bid,
        &config.mid_price_method_ask,
        all_orders.as_deref(),
        config.volume_threshold_usdt,
        config.fix_mid_price,
    )?;

    let current_price = price_result.mid_price;

    println!("\n📊 Price Calculation:");
    println!("  Methods: {}", price_result.method_description);
    println!("{}", price_result.details);
    println!("  => Mid Price: {:.6}", current_price);

    // 根据当前价格选择网格配置
    let grid_config = config.grid.select_config(current_price);

    // 读取上次布单的状态（为每个交易所单独保存状态）
    let state_file = format!(".state_{}_{}.json", exchange_config.name, symbol);
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

            let all_orders = client.get_open_orders(Some(symbol)).await?;
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
    let mut orders = OrderCalculator::calculate_grid_orders(current_price, &grid_config);

    // 获取现有挂单
    println!("\n🔍 Checking existing orders...");
    let existing_orders = client.get_open_orders(Some(symbol)).await?;

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

    // 获取账户余额以检查是否有足够的资金
    println!("\n💰 Checking account balance...");
    let account = client.get_account_info().await?;
    let (base_asset, quote_asset) = client.get_symbol_assets(symbol);

    // 查找 USDT 和基础资产余额
    let mut usdt_available = 0.0;
    let mut base_available = 0.0;
    let mut usdt_locked = 0.0;
    let mut base_locked = 0.0;

    for balance in &account.balances {
        if balance.asset == quote_asset {
            usdt_available = balance.free.parse().unwrap_or(0.0);
            usdt_locked = balance.locked.parse().unwrap_or(0.0);
        } else if balance.asset == base_asset {
            base_available = balance.free.parse().unwrap_or(0.0);
            base_locked = balance.locked.parse().unwrap_or(0.0);
        }
    }

    println!("  Available {} balance: {:.2} (locked: {:.2})", quote_asset, usdt_available, usdt_locked);
    println!("  Available {} balance: {:.8} (locked: {:.8})", base_asset, base_available, base_locked);

    // 计算计划下单需要的资源
    let total_buy_value: f64 = orders.iter()
        .filter(|o| o.side == "BUY")
        .map(|o| o.price * o.quantity)
        .sum();
    let total_sell_quantity: f64 = orders.iter()
        .filter(|o| o.side == "SELL")
        .map(|o| o.quantity)
        .sum();

    println!("  Required for planned orders:");
    println!("    Buy orders need: {:.2} {}", total_buy_value, quote_asset);
    println!("    Sell orders need: {:.8} {}", total_sell_quantity, base_asset);

    // 对订单进行排序，确保最接近市场价格的订单优先处理
    // 买单：价格从高到低（最接近市场价的先）
    // 卖单：价格从低到高（最接近市场价的先）
    let mut sorted_orders = orders.clone();
    sorted_orders.sort_by(|a, b| {
        if a.side == "BUY" && b.side == "BUY" {
            // 买单：价格高的优先
            b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal)
        } else if a.side == "SELL" && b.side == "SELL" {
            // 卖单：价格低的优先
            a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal)
        } else if a.side == "BUY" {
            // 买单在卖单前
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    // 过滤掉余额不足的订单
    let mut filtered_orders = Vec::new();
    let mut skipped_buy_count = 0;
    let mut skipped_sell_count = 0;
    let mut required_usdt = 0.0;
    let mut required_base = 0.0;

    for order in sorted_orders {
        if order.side == "BUY" {
            let order_value = order.price * order.quantity;
            if required_usdt + order_value <= usdt_available {
                required_usdt += order_value;
                filtered_orders.push(order);
            } else {
                skipped_buy_count += 1;
                println!("  ⚠️  Skipping BUY order @ {:.6} (value: {:.2} USDT) - insufficient balance",
                         order.price, order_value);
            }
        } else if order.side == "SELL" {
            if required_base + order.quantity <= base_available {
                required_base += order.quantity;
                filtered_orders.push(order);
            } else {
                skipped_sell_count += 1;
                println!("  ⚠️  Skipping SELL order @ {:.6} (qty: {:.8} {}) - insufficient balance",
                         order.price, order.quantity, base_asset);
            }
        }
    }

    if skipped_buy_count > 0 || skipped_sell_count > 0 {
        println!("\n⚠️  Skipped {} BUY and {} SELL orders due to insufficient balance",
                 skipped_buy_count, skipped_sell_count);
    }

    if filtered_orders.is_empty() {
        println!("\n❌ All orders skipped due to insufficient balance. No orders to place.");
        println!("💡 Please deposit more funds or reduce order sizes in config.");
        return Ok(());
    }

    let orders = filtered_orders;
    let total_orders = orders.len();

    // Convert to BatchOrder format
    let batch_orders: Vec<exchange::BatchOrder> = orders
        .iter()
        .map(|order| exchange::BatchOrder {
            symbol: symbol.to_string(),
            side: order.side.clone(),
            quantity: order.quantity,
            price: order.price,
        })
        .collect();

    println!("\n📦 Using batch order API to place {} orders...", total_orders);

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
        let new_state = TradingState::new(symbol.to_string(), current_price);
        if let Err(e) = new_state.save(&state_file) {
            println!("\n⚠️  Warning: Failed to save state: {}", e);
        } else {
            println!("\n💾 Saved current price state: {:.6}", current_price);
        }
    }

    // 记录账户快照（无论布单是否成功都记录）
    println!("\n📸 Capturing account snapshot...");
    match capture_account_snapshot(&client, &exchange_config.name, symbol, Some(current_price), iteration).await {
        Ok(snapshot) => {
            let history = SnapshotHistory::new(&exchange_config.name, symbol);
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
                        let (base_asset, quote_asset) = client.get_symbol_assets(symbol);
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

        match show_orders_for_exchange(exchange_config, &config, closed).await {
            Ok(_) => {},
            Err(e) => println!("\n❌ Failed to fetch orders from {}: {}", exchange_config.name, e),
        }
    }

    Ok(())
}

async fn show_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    config: &config::Config,
    closed: u32,
) -> Result<()> {
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let symbol = config.grid.get_symbol();

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, symbol).await?;

    // 获取并显示市场深度
    println!("\nFetching market depth...");
    if let Err(e) = show_market_depth(&client, symbol).await {
        println!("⚠️  Warning: Failed to fetch market depth: {}", e);
    }

    // 只查询配置文件中指定的交易对
    println!("\nFetching open orders for {}...", symbol);
    let orders = client.get_open_orders(Some(symbol)).await?;

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
        let all_orders = client.get_all_orders(symbol, Some(limit + 100)).await?;

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

        match cancel_orders_for_exchange(exchange_config, &config, force).await {
            Ok(_) => {},
            Err(e) => println!("\n❌ Failed to cancel orders on {}: {}", exchange_config.name, e),
        }
    }

    Ok(())
}

async fn cancel_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    config: &config::Config,
    force: bool,
) -> Result<()> {
    let symbol = config.grid.get_symbol();
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, symbol).await?;

    println!("Fetching open orders...");
    let orders = client.get_open_orders(Some(symbol)).await?;

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

            match show_pnl_report_for_exchange(exchange_config, &config, recent).await {
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
    config: &config::Config,
    recent: usize,
) -> Result<()> {
    let symbol = config.grid.get_symbol();
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    let (base_asset, quote_asset) = client.get_symbol_assets(symbol);

    // 显示账户信息
    show_account_info(&client, symbol).await?;

    // 显示市场深度
    if let Err(e) = show_market_depth(&client, symbol).await {
        println!("⚠️  Warning: Failed to fetch market depth: {}", e);
    }

    // 显示订单深度统计（不显示具体订单列表）
    if let Err(e) = show_order_depth_summary(&client, symbol).await {
        println!("⚠️  Warning: Failed to fetch order depth: {}", e);
    }

    // 加载快照历史
    let history = SnapshotHistory::new(&exchange_config.name, symbol);
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

        match show_trades_for_exchange(exchange_config, &config, limit).await {
            Ok(_) => {},
            Err(e) => println!("\n❌ Failed to fetch trades from {}: {}", exchange_config.name, e),
        }
    }

    Ok(())
}

async fn show_trades_for_exchange(
    exchange_config: &config::ExchangeConfig,
    config: &config::Config,
    limit: Option<u32>,
) -> Result<()> {
    let symbol = config.grid.get_symbol();
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    // 显示账户信息
    show_account_info(&client, symbol).await?;

    println!("Fetching trades for {}...", symbol);
    let trades = client.get_my_trades(symbol, limit).await?;

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

/// 显示交易统计
async fn show_trade_stats(
    config_path: &str,
    exchange_name: &str,
    limit: usize,
) -> Result<()> {
    use crate::trade_history::TradeHistory;
    use crate::trade_analysis::TradingStats;

    let config = Config::from_file(config_path)?;
    let symbol = config.grid.get_symbol();
    let trade_history = TradeHistory::new(exchange_name, symbol);

    // 加载所有交易
    let mut all_trades = trade_history.load_all()?;

    if all_trades.is_empty() {
        println!("❌ No trade history found for {} - {}", exchange_name, symbol);
        println!("💡 Run 'report' command first to sync trade history from API");
        return Ok(());
    }

    // 按时间排序（最新的在最后）
    all_trades.sort_by_key(|t| t.time);

    // 如果指定了 limit，只取最近的N笔交易
    let trades_to_analyze = if limit > 0 && all_trades.len() > limit {
        println!("📊 Analyzing most recent {} trades (out of {} total)\n", limit, all_trades.len());
        &all_trades[all_trades.len() - limit..]
    } else {
        println!("📊 Analyzing all {} trades\n", all_trades.len());
        &all_trades
    };

    // 获取资产名称
    let (base_asset, quote_asset) = if symbol.ends_with("USDT") {
        let base = symbol.strip_suffix("USDT").unwrap_or(symbol);
        (base.to_string(), "USDT".to_string())
    } else if symbol.ends_with("USDC") {
        let base = symbol.strip_suffix("USDC").unwrap_or(symbol);
        (base.to_string(), "USDC".to_string())
    } else {
        // 默认假设最后4个字符是计价资产
        let len = symbol.len();
        if len > 4 {
            (symbol[..len-4].to_string(), symbol[len-4..].to_string())
        } else {
            (symbol.to_string(), "UNKNOWN".to_string())
        }
    };

    // 计算统计数据
    let stats = TradingStats::from_trades(
        trades_to_analyze,
        symbol,
        &base_asset,
        &quote_asset
    )?;

    // 打印统计信息
    stats.print_summary();

    // 显示时间范围
    if let (Some(first), Some(last)) = (trades_to_analyze.first(), trades_to_analyze.last()) {
        println!("\n📅 Time Range:");
        println!("   First Trade: {}",
            chrono::DateTime::from_timestamp(first.time / 1000, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S UTC"));
        println!("   Last Trade:  {}",
            chrono::DateTime::from_timestamp(last.time / 1000, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S UTC"));
    }

    Ok(())
}

/// 刷量：通过两个账户对敲制造成交量
///
/// direction = "buy": 制造买入成交
///   - 账户1在ask最低价下方挂卖单（maker）
///   - 账户2在相同价格挂买单成交（taker）
///
/// direction = "sell": 制造卖出成交
///   - 账户1在bid最高价上方挂买单（maker）
///   - 账户2在相同价格挂卖单成交（taker）
async fn create_volume(
    config_path: &str,
    exchange: Option<&str>,
    direction: &str,
    value: f64,
    repeat: u32,
    interval: u64,
    toggle: u32,
) -> Result<()> {
    println!("📊 刷量模式 (Create Volume)");
    println!("{}", "═".repeat(60));

    // 验证方向参数
    let initial_direction = direction.to_lowercase();
    if initial_direction != "buy" && initial_direction != "sell" {
        anyhow::bail!("Invalid direction: {}. Must be 'buy' or 'sell'", initial_direction);
    }

    // 加载配置
    let config = Config::from_file(config_path)?;
    let symbol = config.grid.get_symbol().to_string();

    // 获取 volume 配置
    let volume_wrapper = config.volume.as_ref().ok_or_else(|| {
        anyhow::anyhow!("配置文件中缺少 'volume' 字段！请在 config.json 中添加刷量账户配置。\n\n示例:\n\"volume\": {{\n  \"exchange\": \"mexc\",\n  \"account1\": {{\n    \"api_key\": \"账户1的API_KEY\",\n    \"api_secret\": \"账户1的API_SECRET\"\n  }},\n  \"account2\": {{\n    \"api_key\": \"账户2的API_KEY\",\n    \"api_secret\": \"账户2的API_SECRET\"\n  }}\n}}\n\n或者支持多个交易所:\n\"volume\": [\n  {{\n    \"exchange\": \"mexc\",\n    \"account1\": {{ ... }},\n    \"account2\": {{ ... }}\n  }},\n  {{\n    \"exchange\": \"gate\",\n    \"account1\": {{ ... }},\n    \"account2\": {{ ... }}\n  }}\n]")
    })?;

    // 根据 -c 参数选择交易所配置
    let volume_config = volume_wrapper.get_exchange_config(exchange).ok_or_else(|| {
        let available = volume_wrapper.available_exchanges().join(", ");
        if let Some(ex) = exchange {
            anyhow::anyhow!("未找到交易所 '{}' 的刷量配置！\n可用的交易所: {}", ex, available)
        } else {
            anyhow::anyhow!("未找到任何刷量配置！")
        }
    })?;

    // 使用命令行参数的value，如果为0则使用配置中的default_value
    let actual_value = if value > 0.0 { value } else { volume_config.default_value };
    let enable_guard_order = volume_config.enable_guard_order;
    let guard_price_offset = volume_config.guard_price_offset;
    let guard_quantity_percent = volume_config.guard_quantity_percent;

    println!("  初始方向: {} ({})",
        if initial_direction == "buy" { "买入" } else { "卖出" },
        initial_direction
    );
    println!("  目标成交金额: {:.2} USDT", actual_value);
    println!("  定价方式: 中位价 (Mid Price)");
    println!("  防套利阻挡单: {}", if enable_guard_order { "启用" } else { "禁用" });
    if enable_guard_order {
        println!("    - 阻挡单价格偏移: {:.8}", guard_price_offset);
        println!("    - 阻挡单数量: {:.2}% of maker", guard_quantity_percent);
    }
    println!("  交易对: {}", symbol);
    println!("  每轮重复次数: {}", repeat);
    println!("  重复间隔: {} 秒", interval);
    println!("  切换方向次数: {}", toggle);
    if toggle > 0 {
        println!("  总执行次数: {} ({}轮 x {}次切换)", repeat * (toggle + 1), repeat, toggle + 1);
    }

    let exchange_name = &volume_config.exchange;
    println!("  交易所: {}", exchange_name);
    println!("  账户1 (Maker): {}...", &volume_config.account1.api_key[..8.min(volume_config.account1.api_key.len())]);
    println!("  账户2 (Taker): {}...", &volume_config.account2.api_key[..8.min(volume_config.account2.api_key.len())]);
    println!("{}", "═".repeat(60));

    // 创建两个交易所客户端
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_name))
        .context(format!("Invalid exchange type: {}", exchange_name))?;

    let client1 = create_exchange(
        &exchange_type,
        volume_config.account1.api_key.clone(),
        volume_config.account1.api_secret.clone(),
        volume_config.account1.api_passphrase.clone(),
    )?;

    let client2 = create_exchange(
        &exchange_type,
        volume_config.account2.api_key.clone(),
        volume_config.account2.api_secret.clone(),
        volume_config.account2.api_passphrase.clone(),
    )?;

    // 检查两个账户的残余挂单
    println!("\n📋 检查账户残余挂单...");
    let open_orders1 = client1.get_open_orders(Some(&symbol)).await?;
    let open_orders2 = client2.get_open_orders(Some(&symbol)).await?;

    if open_orders1.is_empty() && open_orders2.is_empty() {
        println!("  ✅ 两个账户均无残余挂单");
    } else {
        if !open_orders1.is_empty() {
            println!("  ⚠️  账户1 有 {} 个残余挂单:", open_orders1.len());
            for order in &open_orders1 {
                let price: f64 = order.price.parse().unwrap_or(0.0);
                let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
                let executed: f64 = order.executed_qty.parse().unwrap_or(0.0);
                println!("      {} | 价格: {:.6} | 数量: {:.4} | 已成交: {:.4} | ID: {}",
                    order.side, price, qty, executed, order.order_id);
            }
        } else {
            println!("  ✅ 账户1 无残余挂单");
        }

        if !open_orders2.is_empty() {
            println!("  ⚠️  账户2 有 {} 个残余挂单:", open_orders2.len());
            for order in &open_orders2 {
                let price: f64 = order.price.parse().unwrap_or(0.0);
                let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
                let executed: f64 = order.executed_qty.parse().unwrap_or(0.0);
                println!("      {} | 价格: {:.6} | 数量: {:.4} | 已成交: {:.4} | ID: {}",
                    order.side, price, qty, executed, order.order_id);
            }
        } else {
            println!("  ✅ 账户2 无残余挂单");
        }
    }

    // 获取订单簿
    println!("\n📖 获取订单簿...");
    let order_book = client1.get_order_book(&symbol, Some(5)).await?;

    if order_book.bids.is_empty() || order_book.asks.is_empty() {
        anyhow::bail!("Order book is empty");
    }

    let best_bid: f64 = order_book.bids[0][0].parse().context("Failed to parse best bid")?;
    let best_ask: f64 = order_book.asks[0][0].parse().context("Failed to parse best ask")?;

    println!("  Best Bid: {:.6}", best_bid);
    println!("  Best Ask: {:.6}", best_ask);
    println!("  Spread: {:.6} ({:.4}%)", best_ask - best_bid, (best_ask - best_bid) / best_bid * 100.0);

    // 计算对敲价格（使用中位价）
    let mid_price = (best_bid + best_ask) / 2.0;
    println!("  Mid Price: {:.6}", mid_price);

    let trade_price = mid_price;

    // 确保价格在bid-ask之间（安全区间）
    let trade_price = if trade_price < best_bid {
        println!("⚠️  计算价格 {:.6} 低于 best_bid，调整为 best_bid", trade_price);
        best_bid
    } else if trade_price > best_ask {
        println!("⚠️  计算价格 {:.6} 高于 best_ask，调整为 best_ask", trade_price);
        best_ask
    } else {
        trade_price
    };

    // 计算数量
    let quantity = actual_value / trade_price;

    println!("\n💰 对敲参数:");
    println!("  成交价格: {:.6}", trade_price);
    println!("  成交数量: {:.6}", quantity);
    println!("  成交金额: {:.2} USDT", trade_price * quantity);

    // 获取资产名称
    let (base_asset, quote_asset) = client1.get_symbol_assets(&symbol);

    // 检查账户余额
    println!("\n💼 检查账户余额...");

    let account1_info = client1.get_account_info().await?;
    let account2_info = client2.get_account_info().await?;

    // 账户1余额
    let mut account1_base = 0.0;
    let mut account1_quote = 0.0;
    for balance in &account1_info.balances {
        if balance.asset == base_asset {
            account1_base = balance.free.parse().unwrap_or(0.0);
        } else if balance.asset == quote_asset {
            account1_quote = balance.free.parse().unwrap_or(0.0);
        }
    }

    // 账户2余额
    let mut account2_base = 0.0;
    let mut account2_quote = 0.0;
    for balance in &account2_info.balances {
        if balance.asset == base_asset {
            account2_base = balance.free.parse().unwrap_or(0.0);
        } else if balance.asset == quote_asset {
            account2_quote = balance.free.parse().unwrap_or(0.0);
        }
    }

    println!("  账户1: {:.4} {} | {:.2} {}", account1_base, base_asset, account1_quote, quote_asset);
    println!("  账户2: {:.4} {} | {:.2} {}", account2_base, base_asset, account2_quote, quote_asset);

    // 验证余额（检查两个方向的余额，因为toggle会切换方向）
    if toggle > 0 {
        // 如果有toggle，需要两个方向的余额都足够
        if account1_base < quantity {
            anyhow::bail!(
                "账户1 {} 余额不足！需要 {:.6}，可用 {:.6}",
                base_asset, quantity, account1_base
            );
        }
        if account1_quote < actual_value {
            anyhow::bail!(
                "账户1 {} 余额不足！需要 {:.2}，可用 {:.2}",
                quote_asset, actual_value, account1_quote
            );
        }
        if account2_base < quantity {
            anyhow::bail!(
                "账户2 {} 余额不足！需要 {:.6}，可用 {:.6}",
                base_asset, quantity, account2_base
            );
        }
        if account2_quote < actual_value {
            anyhow::bail!(
                "账户2 {} 余额不足！需要 {:.2}，可用 {:.2}",
                quote_asset, actual_value, account2_quote
            );
        }
    } else if initial_direction == "buy" {
        // 买入成交：账户1卖出base，账户2买入base
        if account1_base < quantity {
            anyhow::bail!(
                "账户1 {} 余额不足！需要 {:.6}，可用 {:.6}",
                base_asset, quantity, account1_base
            );
        }
        if account2_quote < actual_value {
            anyhow::bail!(
                "账户2 {} 余额不足！需要 {:.2}，可用 {:.2}",
                quote_asset, actual_value, account2_quote
            );
        }
    } else {
        // 卖出成交：账户1买入base，账户2卖出base
        if account1_quote < actual_value {
            anyhow::bail!(
                "账户1 {} 余额不足！需要 {:.2}，可用 {:.2}",
                quote_asset, actual_value, account1_quote
            );
        }
        if account2_base < quantity {
            anyhow::bail!(
                "账户2 {} 余额不足！需要 {:.6}，可用 {:.6}",
                base_asset, quantity, account2_base
            );
        }
    }

    println!("\n✅ 余额检查通过");

    // 记录初始总余额（用于后续比较）
    let initial_total_base = account1_base + account2_base;
    let initial_total_quote = account1_quote + account2_quote;
    let mut prev_total_base = initial_total_base;
    let mut prev_total_quote = initial_total_quote;

    println!("\n📊 初始总余额:");
    println!("  {} 总量: {:.8}", base_asset, initial_total_base);
    println!("  {} 总量: {:.8}", quote_asset, initial_total_quote);

    // 确认操作
    println!("\n{}", "═".repeat(60));
    println!("⚠️  即将执行对敲交易！");
    println!("{}", "═".repeat(60));

    if initial_direction == "buy" {
        println!("  首轮步骤1: 账户1 挂卖单 {:.6} {} @ {:.6}", quantity, base_asset, trade_price);
        println!("  首轮步骤2: 账户2 挂买单 {:.6} {} @ {:.6} (吃掉账户1的卖单)", quantity, base_asset, trade_price);
    } else {
        println!("  首轮步骤1: 账户1 挂买单 {:.6} {} @ {:.6}", quantity, base_asset, trade_price);
        println!("  首轮步骤2: 账户2 挂卖单 {:.6} {} @ {:.6} (吃掉账户1的买单)", quantity, base_asset, trade_price);
    }
    if toggle > 0 {
        println!("  (之后将切换方向 {} 次)", toggle);
    }

    // 如果重复次数大于1或toggle大于0，跳过确认直接执行
    if repeat == 1 && toggle == 0 {
        println!("\n❓ 确认执行？输入 'yes' 继续: ");
        print!("> ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "yes" {
            println!("\n❌ 操作已取消");
            return Ok(());
        }
    } else {
        let total_rounds = repeat * (toggle + 1);
        println!("\n🔄 批量模式 (共 {} 次执行)，跳过确认直接执行...", total_rounds);
    }

    // 外层循环：处理toggle切换
    let mut current_direction = initial_direction.clone();
    for toggle_round in 0..=toggle {
        if toggle > 0 {
            println!("\n{}", "═".repeat(60));
            println!("🔀 第 {}/{} 轮方向: {} ({})",
                toggle_round + 1,
                toggle + 1,
                if current_direction == "buy" { "买入" } else { "卖出" },
                current_direction
            );
            println!("{}", "═".repeat(60));
        }

        // 内层循环：执行repeat次（使用while循环，失败时不递增round，实现重试）
        let mut round = 1u32;
        while round <= repeat {
            if repeat > 1 {
                println!("\n{}", "─".repeat(40));
                println!("📊 第 {}/{} 次执行", round, repeat);
                println!("{}", "─".repeat(40));
            }

            // 每轮重新获取订单簿（带重试）
            println!("\n📖 获取最新订单簿...");
            let order_book = loop {
                match client1.get_order_book(&symbol, Some(5)).await {
                    Ok(ob) => break ob,
                    Err(e) => {
                        println!("  ⚠️  获取订单簿失败: {}", e);
                        println!("  🔄 20秒后重试...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                        continue;
                    }
                }
            };

            if order_book.bids.is_empty() || order_book.asks.is_empty() {
                println!("  ❌ Order book is empty");
                println!("\n🔄 等待 1 秒后重试第 {}/{} 次执行...", round, repeat);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }

            let best_bid: f64 = order_book.bids[0][0].parse().context("Failed to parse best bid")?;
            let best_ask: f64 = order_book.asks[0][0].parse().context("Failed to parse best ask")?;

            println!("  Best Bid: {:.6}", best_bid);
            println!("  Best Ask: {:.6}", best_ask);

            // 重新计算对敲价格（使用中位价）
            let mid_price = (best_bid + best_ask) / 2.0;
            println!("  Mid Price: {:.6}", mid_price);
            let trade_price = mid_price;

            let trade_price = if trade_price < best_bid {
                best_bid
            } else if trade_price > best_ask {
                best_ask
            } else {
                trade_price
            };

            let quantity = actual_value / trade_price;

            println!("  成交价格: {:.6}", trade_price);
            println!("  成交数量: {:.6}", quantity);

            println!("\n🚀 开始执行对敲...");

            // 保存订单信息用于最后打印
            let (maker_order, taker_order, guard_order_id) = if current_direction == "buy" {
                // 步骤1: 账户1挂卖单（maker）+ 阻挡买单（防套利）- 带网络重试
                // 阻挡买单价格微微低于maker卖单，挡在卖单下方保护maker单
                let guard_buy_price = trade_price - guard_price_offset;
                let guard_quantity = quantity * guard_quantity_percent / 100.0;

                let (maker, guard_id) = if enable_guard_order {
                    println!("\n📤 步骤1: 账户1 Batch挂卖单 + 阻挡买单...");
                    println!("    - Maker卖单: 价格={:.6}, 数量={:.6}", trade_price, quantity);
                    println!("    - 阻挡买单: 价格={:.6}, 数量={:.6} ({:.2}%)", guard_buy_price, guard_quantity, guard_quantity_percent);

                    let batch_orders = vec![
                        BatchOrder {
                            symbol: symbol.clone(),
                            side: "SELL".to_string(),
                            quantity,
                            price: trade_price,
                        },
                        BatchOrder {
                            symbol: symbol.clone(),
                            side: "BUY".to_string(),
                            quantity: guard_quantity,
                            price: guard_buy_price,
                        },
                    ];

                    let batch_result = loop {
                        match client1.place_batch_limit_orders(batch_orders.clone()).await {
                            Ok(results) => break results,
                            Err(e) => {
                                let err_str = e.to_string();
                                if err_str.contains("dns error") || err_str.contains("error sending request") || err_str.contains("connection") {
                                    println!("  ⚠️  网络错误: {}", e);
                                    println!("  🔄 20秒后重试...");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                                    continue;
                                } else {
                                    return Err(e);
                                }
                            }
                        }
                    };

                    // 解析batch结果
                    let mut batch_iter = batch_result.into_iter();
                    let maker_result = batch_iter.next().ok_or_else(|| anyhow::anyhow!("Batch返回结果为空"))?;
                    let guard_result = batch_iter.next();

                    let maker = match maker_result {
                        Ok(order) => {
                            println!("  ✅ Maker卖单已挂出，订单ID: {}", order.order_id);
                            order
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!("Maker卖单下单失败: {}", e));
                        }
                    };

                    let guard_id = match guard_result {
                        Some(Ok(order)) => {
                            println!("  ✅ 阻挡买单已挂出，订单ID: {}", order.order_id);
                            Some(order.order_id)
                        }
                        Some(Err(e)) => {
                            println!("  ⚠️  阻挡买单失败: {}（继续执行）", e);
                            None
                        }
                        None => {
                            println!("  ⚠️  阻挡买单无返回（继续执行）");
                            None
                        }
                    };

                    (maker, guard_id)
                } else {
                    // 不启用阻挡单，使用原有逻辑
                    println!("\n📤 步骤1: 账户1 挂卖单...");
                    let maker = loop {
                        match client1.place_limit_order(&symbol, "SELL", quantity, trade_price).await {
                            Ok(order) => break order,
                            Err(e) => {
                                let err_str = e.to_string();
                                if err_str.contains("dns error") || err_str.contains("error sending request") || err_str.contains("connection") {
                                    println!("  ⚠️  网络错误: {}", e);
                                    println!("  🔄 20秒后重试...");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                                    continue;
                                } else {
                                    return Err(e);
                                }
                            }
                        }
                    };
                    println!("  ✅ 卖单已挂出，订单ID: {}", maker.order_id);
                    (maker, None::<String>)
                };

                // 短暂等待确保订单进入订单簿
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // 检查maker单是否是最低卖价（best ask）且没有同价位的其他单
                println!("\n🔍 检查maker单是否为最优价...");

                // 查询我们的 maker 订单，获取交易所返回的准确价格字符串
                let maker_order_info = client1.get_order(&symbol, &maker.order_id).await.ok();
                let maker_price_str = maker_order_info.as_ref().map(|o| o.price.as_str()).unwrap_or("");
                let maker_qty: f64 = maker_order_info.as_ref()
                    .and_then(|o| o.orig_qty.parse().ok())
                    .unwrap_or(quantity);

                let check_order_book = match client1.get_order_book(&symbol, Some(5)).await {
                    Ok(ob) => ob,
                    Err(e) => {
                        println!("  ⚠️  获取订单簿失败: {}，跳过检查", e);
                        // 获取失败时继续执行，不撤单
                        OrderBook { bids: vec![], asks: vec![] }
                    }
                };

                if !check_order_book.asks.is_empty() && !maker_price_str.is_empty() {
                    let best_ask_price_str = &check_order_book.asks[0][0];
                    let best_ask_qty: f64 = check_order_book.asks[0][1].parse().unwrap_or(0.0);
                    println!("  当前最低卖价: {}，数量: {:.6}", best_ask_price_str, best_ask_qty);
                    println!("  Maker卖单价格: {}，数量: {:.6} (订单ID: {})", maker_price_str, maker_qty, maker.order_id);

                    // 直接用字符串比较价格，避免浮点精度问题
                    let need_retry = if maker_price_str == best_ask_price_str {
                        // 价格完全相同，检查是否有其他同价订单
                        if best_ask_qty > maker_qty * 1.01 {
                            println!("  ❌ 同价位有其他卖单！(订单簿数量 {:.6} > 我们的 {:.6})", best_ask_qty, maker_qty);
                            true
                        } else {
                            println!("  ✅ Maker卖单是最优价");
                            false
                        }
                    } else {
                        // 价格不同，比较大小
                        let maker_price_f64: f64 = maker_price_str.parse().unwrap_or(0.0);
                        let best_ask_f64: f64 = best_ask_price_str.parse().unwrap_or(0.0);
                        if maker_price_f64 > best_ask_f64 {
                            println!("  ❌ Maker卖单不是最低卖价！有其他卖单价格更低");
                            println!("  📋 阻挡的卖单:");
                            for (i, ask) in check_order_book.asks.iter().enumerate() {
                                let ask_price_str = &ask[0];
                                let ask_price: f64 = ask_price_str.parse().unwrap_or(0.0);
                                let ask_qty: f64 = ask[1].parse().unwrap_or(0.0);
                                if ask_price < maker_price_f64 {
                                    println!("      #{}: 价格 {}, 数量 {:.6}", i + 1, ask_price_str, ask_qty);
                                } else {
                                    break;
                                }
                            }
                            true
                        } else {
                            println!("  ✅ Maker卖单是最优价");
                            false
                        }
                    };

                    if need_retry {
                        // 检查阻挡买单状态，诊断为什么没有阻挡住
                        if let Some(ref gid) = guard_id {
                            match client1.get_order(&symbol, gid).await {
                                Ok(guard_order) => {
                                    let executed: f64 = guard_order.executed_qty.parse().unwrap_or(0.0);
                                    let orig_qty: f64 = guard_order.orig_qty.parse().unwrap_or(0.0);
                                    if executed > 0.0 {
                                        println!("  📊 阻挡买单状态: {} (成交 {:.6} / {:.6})", guard_order.status, executed, orig_qty);
                                        println!("      → 阻挡单被其他卖单吃掉了，更低价的卖单是之后挂出的");
                                    } else {
                                        println!("  📊 阻挡买单状态: {} (未成交)", guard_order.status);
                                        println!("      → 更低价的卖单可能使用了POST_ONLY，或在阻挡单之前就存在");
                                    }
                                }
                                Err(e) => {
                                    println!("  ⚠️  查询阻挡买单状态失败: {}", e);
                                }
                            }
                        }

                        println!("  🔄 撤单并重新计算价格...");
                        // 撤销maker单
                        if let Err(cancel_err) = client1.cancel_order(&symbol, &maker.order_id).await {
                            println!("  ⚠️  撤销Maker单失败: {}", cancel_err);
                        } else {
                            println!("  ✅ Maker单已撤销");
                        }
                        // 撤销阻挡单
                        if let Some(ref gid) = guard_id {
                            if let Err(cancel_err) = client1.cancel_order(&symbol, gid).await {
                                println!("  ⚠️  撤销阻挡单失败: {}", cancel_err);
                            } else {
                                println!("  ✅ 阻挡单已撤销");
                            }
                        }
                        // 短暂等待后重试当前执行
                        println!("\n🔄 等待 1 秒后重试第 {}/{} 次执行...", round, repeat);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                    println!("  ✅ Maker卖单是唯一最低卖价，继续执行");
                }

                // 步骤2: 账户2挂买单（taker）- 带网络重试
                println!("\n📥 步骤2: 账户2 挂买单（吃单）...");
                let taker_result = loop {
                    match client2.place_limit_order(&symbol, "BUY", quantity, trade_price).await {
                        Ok(order) => break Ok(order),
                        Err(e) => {
                            let err_str = e.to_string();
                            if err_str.contains("dns error") || err_str.contains("error sending request") || err_str.contains("connection") {
                                println!("  ⚠️  网络错误: {}", e);
                                println!("  🔄 20秒后重试...");
                                tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                                continue;
                            } else {
                                break Err(e);
                            }
                        }
                    }
                };
                match taker_result {
                    Ok(taker) => (maker, taker, guard_id),
                    Err(e) => {
                        println!("  ❌ 买单失败: {}", e);
                        println!("  ⚠️  正在取消账户1的挂单...");
                        if let Err(cancel_err) = client1.cancel_order(&symbol, &maker.order_id).await {
                            println!("  ❌ 取消Maker挂单失败: {}", cancel_err);
                        } else {
                            println!("  ✅ Maker挂单已取消");
                        }
                        // 取消阻挡单
                        if let Some(ref gid) = guard_id {
                            if let Err(cancel_err) = client1.cancel_order(&symbol, gid).await {
                                println!("  ❌ 取消阻挡单失败: {}", cancel_err);
                            } else {
                                println!("  ✅ 阻挡单已取消");
                            }
                        }
                        // 失败后等待1秒重试当前执行
                        println!("\n🔄 等待 1 秒后重试第 {}/{} 次执行...", round, repeat);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                }
            } else {
                // 步骤1: 账户1挂买单（maker）+ 阻挡卖单（防套利）- 带网络重试
                // 阻挡卖单价格微微高于maker买单，挡在买单上方保护maker单
                let guard_sell_price = trade_price + guard_price_offset;
                let guard_quantity = quantity * guard_quantity_percent / 100.0;

                let (maker, guard_id) = if enable_guard_order {
                    println!("\n📥 步骤1: 账户1 Batch挂买单 + 阻挡卖单...");
                    println!("    - Maker买单: 价格={:.6}, 数量={:.6}", trade_price, quantity);
                    println!("    - 阻挡卖单: 价格={:.6}, 数量={:.6} ({:.2}%)", guard_sell_price, guard_quantity, guard_quantity_percent);

                    let batch_orders = vec![
                        BatchOrder {
                            symbol: symbol.clone(),
                            side: "BUY".to_string(),
                            quantity,
                            price: trade_price,
                        },
                        BatchOrder {
                            symbol: symbol.clone(),
                            side: "SELL".to_string(),
                            quantity: guard_quantity,
                            price: guard_sell_price,
                        },
                    ];

                    let batch_result = loop {
                        match client1.place_batch_limit_orders(batch_orders.clone()).await {
                            Ok(results) => break results,
                            Err(e) => {
                                let err_str = e.to_string();
                                if err_str.contains("dns error") || err_str.contains("error sending request") || err_str.contains("connection") {
                                    println!("  ⚠️  网络错误: {}", e);
                                    println!("  🔄 20秒后重试...");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                                    continue;
                                } else {
                                    return Err(e);
                                }
                            }
                        }
                    };

                    // 解析batch结果
                    let mut batch_iter = batch_result.into_iter();
                    let maker_result = batch_iter.next().ok_or_else(|| anyhow::anyhow!("Batch返回结果为空"))?;
                    let guard_result = batch_iter.next();

                    let maker = match maker_result {
                        Ok(order) => {
                            println!("  ✅ Maker买单已挂出，订单ID: {}", order.order_id);
                            order
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!("Maker买单下单失败: {}", e));
                        }
                    };

                    let guard_id = match guard_result {
                        Some(Ok(order)) => {
                            println!("  ✅ 阻挡卖单已挂出，订单ID: {}", order.order_id);
                            Some(order.order_id)
                        }
                        Some(Err(e)) => {
                            println!("  ⚠️  阻挡卖单失败: {}（继续执行）", e);
                            None
                        }
                        None => {
                            println!("  ⚠️  阻挡卖单无返回（继续执行）");
                            None
                        }
                    };

                    (maker, guard_id)
                } else {
                    // 不启用阻挡单，使用原有逻辑
                    println!("\n📥 步骤1: 账户1 挂买单...");
                    let maker = loop {
                        match client1.place_limit_order(&symbol, "BUY", quantity, trade_price).await {
                            Ok(order) => break order,
                            Err(e) => {
                                let err_str = e.to_string();
                                if err_str.contains("dns error") || err_str.contains("error sending request") || err_str.contains("connection") {
                                    println!("  ⚠️  网络错误: {}", e);
                                    println!("  🔄 20秒后重试...");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                                    continue;
                                } else {
                                    return Err(e);
                                }
                            }
                        }
                    };
                    println!("  ✅ 买单已挂出，订单ID: {}", maker.order_id);
                    (maker, None::<String>)
                };

                // 短暂等待确保订单进入订单簿
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // 检查maker单是否是最高买价（best bid）且没有同价位的其他单
                println!("\n🔍 检查maker单是否为最优价...");

                // 查询我们的 maker 订单，获取交易所返回的准确价格字符串
                let maker_order_info = client1.get_order(&symbol, &maker.order_id).await.ok();
                let maker_price_str = maker_order_info.as_ref().map(|o| o.price.as_str()).unwrap_or("");
                let maker_qty: f64 = maker_order_info.as_ref()
                    .and_then(|o| o.orig_qty.parse().ok())
                    .unwrap_or(quantity);

                let check_order_book = match client1.get_order_book(&symbol, Some(5)).await {
                    Ok(ob) => ob,
                    Err(e) => {
                        println!("  ⚠️  获取订单簿失败: {}，跳过检查", e);
                        // 获取失败时继续执行，不撤单
                        OrderBook { bids: vec![], asks: vec![] }
                    }
                };

                if !check_order_book.bids.is_empty() && !maker_price_str.is_empty() {
                    let best_bid_price_str = &check_order_book.bids[0][0];
                    let best_bid_qty: f64 = check_order_book.bids[0][1].parse().unwrap_or(0.0);
                    println!("  当前最高买价: {}，数量: {:.6}", best_bid_price_str, best_bid_qty);
                    println!("  Maker买单价格: {}，数量: {:.6} (订单ID: {})", maker_price_str, maker_qty, maker.order_id);

                    // 直接用字符串比较价格，避免浮点精度问题
                    let need_retry = if maker_price_str == best_bid_price_str {
                        // 价格完全相同，检查是否有其他同价订单
                        if best_bid_qty > maker_qty * 1.01 {
                            println!("  ❌ 同价位有其他买单！(订单簿数量 {:.6} > 我们的 {:.6})", best_bid_qty, maker_qty);
                            true
                        } else {
                            println!("  ✅ Maker买单是最优价");
                            false
                        }
                    } else {
                        // 价格不同，比较大小
                        let maker_price_f64: f64 = maker_price_str.parse().unwrap_or(0.0);
                        let best_bid_f64: f64 = best_bid_price_str.parse().unwrap_or(0.0);
                        if maker_price_f64 < best_bid_f64 {
                            println!("  ❌ Maker买单不是最高买价！有其他买单价格更高");
                            true
                        } else {
                            println!("  ✅ Maker买单是最优价");
                            false
                        }
                    };

                    if need_retry {
                        // 检查阻挡卖单状态，诊断为什么没有阻挡住
                        if let Some(ref gid) = guard_id {
                            match client1.get_order(&symbol, gid).await {
                                Ok(guard_order) => {
                                    let executed: f64 = guard_order.executed_qty.parse().unwrap_or(0.0);
                                    let orig_qty: f64 = guard_order.orig_qty.parse().unwrap_or(0.0);
                                    if executed > 0.0 {
                                        println!("  📊 阻挡卖单状态: {} (成交 {:.6} / {:.6})", guard_order.status, executed, orig_qty);
                                        println!("      → 阻挡单被其他买单吃掉了，更高价的买单是之后挂出的");
                                    } else {
                                        println!("  📊 阻挡卖单状态: {} (未成交)", guard_order.status);
                                        println!("      → 更高价的买单可能使用了POST_ONLY，或在阻挡单之前就存在");
                                    }
                                }
                                Err(e) => {
                                    println!("  ⚠️  查询阻挡卖单状态失败: {}", e);
                                }
                            }
                        }

                        println!("  🔄 撤单并重新计算价格...");
                        // 撤销maker单
                        if let Err(cancel_err) = client1.cancel_order(&symbol, &maker.order_id).await {
                            println!("  ⚠️  撤销Maker单失败: {}", cancel_err);
                        } else {
                            println!("  ✅ Maker单已撤销");
                        }
                        // 撤销阻挡单
                        if let Some(ref gid) = guard_id {
                            if let Err(cancel_err) = client1.cancel_order(&symbol, gid).await {
                                println!("  ⚠️  撤销阻挡单失败: {}", cancel_err);
                            } else {
                                println!("  ✅ 阻挡单已撤销");
                            }
                        }
                        // 短暂等待后重试当前执行
                        println!("\n🔄 等待 1 秒后重试第 {}/{} 次执行...", round, repeat);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                    println!("  ✅ Maker买单是唯一最高买价，继续执行");
                }

                // 步骤2: 账户2挂卖单（taker）- 带网络重试
                println!("\n📤 步骤2: 账户2 挂卖单（吃单）...");
                let taker_result = loop {
                    match client2.place_limit_order(&symbol, "SELL", quantity, trade_price).await {
                        Ok(order) => break Ok(order),
                        Err(e) => {
                            let err_str = e.to_string();
                            if err_str.contains("dns error") || err_str.contains("error sending request") || err_str.contains("connection") {
                                println!("  ⚠️  网络错误: {}", e);
                                println!("  🔄 20秒后重试...");
                                tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                                continue;
                            } else {
                                break Err(e);
                            }
                        }
                    }
                };
                match taker_result {
                    Ok(taker) => (maker, taker, guard_id),
                    Err(e) => {
                        println!("  ❌ 卖单失败: {}", e);
                        println!("  ⚠️  正在取消账户1的挂单...");
                        if let Err(cancel_err) = client1.cancel_order(&symbol, &maker.order_id).await {
                            println!("  ❌ 取消Maker挂单失败: {}", cancel_err);
                        } else {
                            println!("  ✅ Maker挂单已取消");
                        }
                        // 取消阻挡单
                        if let Some(ref gid) = guard_id {
                            if let Err(cancel_err) = client1.cancel_order(&symbol, gid).await {
                                println!("  ❌ 取消阻挡单失败: {}", cancel_err);
                            } else {
                                println!("  ✅ 阻挡单已取消");
                            }
                        }
                        // 失败后等待1秒重试当前执行
                        println!("\n🔄 等待 1 秒后重试第 {}/{} 次执行...", round, repeat);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                }
            };

            // 等待订单处理
            println!("\n🔍 检查订单成交状态...");
            println!("  Maker 订单ID: {}", maker_order.order_id);
            println!("  Taker 订单ID: {}", taker_order.order_id);
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            // 取消阻挡单（无论成交与否都应取消）
            if let Some(ref gid) = guard_order_id {
                println!("\n🛡️ 取消阻挡单...");
                match client1.cancel_order(&symbol, gid).await {
                    Ok(_) => println!("  ✅ 阻挡单已取消 (ID: {})", gid),
                    Err(e) => {
                        // 可能已经成交或已取消
                        let err_str = e.to_string().to_lowercase();
                        if err_str.contains("filled") || err_str.contains("已成交") || err_str.contains("not exist") || err_str.contains("order_not_found") {
                            println!("  ⚠️  阻挡单已成交或不存在（可能被套利者触发）");
                        } else {
                            println!("  ⚠️  取消阻挡单失败: {}", e);
                        }
                    }
                }
            }

            // 查询 Maker 订单状态（带超时，超时则取消订单）
            println!("  查询 Maker 订单状态...");
            let maker_status = match tokio::time::timeout(
                tokio::time::Duration::from_secs(30),
                client1.get_order(&symbol, &maker_order.order_id)
            ).await {
                Ok(Ok(order)) => Some(order),
                Ok(Err(e)) => {
                    println!("  ⚠️  查询 Maker 订单失败: {}", e);
                    println!("  🔄 尝试取消 Maker 订单...");
                    let _ = client1.cancel_order(&symbol, &maker_order.order_id).await;
                    None
                }
                Err(_) => {
                    println!("  ⚠️  查询 Maker 订单超时（30秒）");
                    println!("  🔄 尝试取消 Maker 订单...");
                    match client1.cancel_order(&symbol, &maker_order.order_id).await {
                        Ok(_) => println!("  ✅ Maker 订单已取消"),
                        Err(e) => println!("  ❌ 取消 Maker 订单失败: {}", e),
                    }
                    None
                }
            };

            // 查询 Taker 订单状态（带超时，超时则取消订单）
            println!("  查询 Taker 订单状态...");
            let taker_status = match tokio::time::timeout(
                tokio::time::Duration::from_secs(30),
                client2.get_order(&symbol, &taker_order.order_id)
            ).await {
                Ok(Ok(order)) => Some(order),
                Ok(Err(e)) => {
                    println!("  ⚠️  查询 Taker 订单失败: {}", e);
                    println!("  🔄 尝试取消 Taker 订单...");
                    let _ = client2.cancel_order(&symbol, &taker_order.order_id).await;
                    None
                }
                Err(_) => {
                    println!("  ⚠️  查询 Taker 订单超时（30秒）");
                    println!("  🔄 尝试取消 Taker 订单...");
                    match client2.cancel_order(&symbol, &taker_order.order_id).await {
                        Ok(_) => println!("  ✅ Taker 订单已取消"),
                        Err(e) => println!("  ❌ 取消 Taker 订单失败: {}", e),
                    }
                    None
                }
            };

            // 打印详细订单信息
            println!("\n{}", "─".repeat(40));
            println!("🎉 本次对敲完成！");
            println!("{}", "─".repeat(40));

            println!("\n📋 Maker 订单 (账户1):");
            println!("  订单ID:    {}", maker_order.order_id);
            println!("  方向:      {}", maker_order.side);
            println!("  价格:      {}", maker_order.price);
            println!("  下单数量:  {}", maker_order.orig_qty);
            if let Some(ref status) = maker_status {
                println!("  成交数量:  {}", status.executed_qty);
                println!("  订单状态:  {}", status.status);
            }

            println!("\n📋 Taker 订单 (账户2):");
            println!("  订单ID:    {}", taker_order.order_id);
            println!("  方向:      {}", taker_order.side);
            println!("  价格:      {}", taker_order.price);
            println!("  下单数量:  {}", taker_order.orig_qty);
            if let Some(ref status) = taker_status {
                println!("  成交数量:  {}", status.executed_qty);
                println!("  订单状态:  {}", status.status);
            }

            // 检查并取消未完全成交的订单
            let mut need_cancel = false;

            if let Some(ref status) = maker_status {
                if status.status != "FILLED" && status.status != "CANCELED" {
                    println!("\n  ⚠️  Maker订单未完全成交 (状态: {})", status.status);
                    need_cancel = true;
                    match client1.cancel_order(&symbol, &maker_order.order_id).await {
                        Ok(_) => println!("  ✅ Maker订单已取消"),
                        Err(e) => println!("  ❌ 取消Maker订单失败: {}", e),
                    }
                }
            }

            if let Some(ref status) = taker_status {
                if status.status != "FILLED" && status.status != "CANCELED" {
                    println!("\n  ⚠️  Taker订单未完全成交 (状态: {})", status.status);
                    need_cancel = true;
                    match client2.cancel_order(&symbol, &taker_order.order_id).await {
                        Ok(_) => println!("  ✅ Taker订单已取消"),
                        Err(e) => println!("  ❌ 取消Taker订单失败: {}", e),
                    }
                }
            }

            if !need_cancel {
                println!("\n  ✅ 两个订单都已完全成交");
            }

            println!("\n📊 本次成交汇总:");
            println!("  成交方向:  {}", if current_direction == "buy" { "买入" } else { "卖出" });
            println!("  成交价格:  {:.8}", trade_price);
            println!("  成交数量:  {:.8} {}", quantity, base_asset);
            println!("  成交金额:  {:.4} {}", trade_price * quantity, quote_asset);

            // 查询当前两个账户的余额总量
            println!("\n💰 余额变化检查...");
            let acc1_info = client1.get_account_info().await?;
            let acc2_info = client2.get_account_info().await?;

            let mut curr_acc1_base = 0.0;
            let mut curr_acc1_quote = 0.0;
            for balance in &acc1_info.balances {
                if balance.asset == base_asset {
                    curr_acc1_base = balance.free.parse().unwrap_or(0.0);
                } else if balance.asset == quote_asset {
                    curr_acc1_quote = balance.free.parse().unwrap_or(0.0);
                }
            }

            let mut curr_acc2_base = 0.0;
            let mut curr_acc2_quote = 0.0;
            for balance in &acc2_info.balances {
                if balance.asset == base_asset {
                    curr_acc2_base = balance.free.parse().unwrap_or(0.0);
                } else if balance.asset == quote_asset {
                    curr_acc2_quote = balance.free.parse().unwrap_or(0.0);
                }
            }

            let curr_total_base = curr_acc1_base + curr_acc2_base;
            let curr_total_quote = curr_acc1_quote + curr_acc2_quote;

            // 计算与上一轮的差值
            let diff_base = curr_total_base - prev_total_base;
            let diff_quote = curr_total_quote - prev_total_quote;

            // 计算与初始的差值
            let diff_from_initial_base = curr_total_base - initial_total_base;
            let diff_from_initial_quote = curr_total_quote - initial_total_quote;

            println!("  账户1: {:.8} {} | {:.4} {}", curr_acc1_base, base_asset, curr_acc1_quote, quote_asset);
            println!("  账户2: {:.8} {} | {:.4} {}", curr_acc2_base, base_asset, curr_acc2_quote, quote_asset);
            println!("  ────────────────────────────────────");
            println!("  总量:   {:.8} {} | {:.4} {}", curr_total_base, base_asset, curr_total_quote, quote_asset);

            // 显示本轮变化
            let base_sign = if diff_base >= 0.0 { "+" } else { "" };
            let quote_sign = if diff_quote >= 0.0 { "+" } else { "" };
            println!("  本轮变化: {}{:.8} {} | {}{:.4} {}",
                base_sign, diff_base, base_asset,
                quote_sign, diff_quote, quote_asset);

            // 显示累计变化
            let init_base_sign = if diff_from_initial_base >= 0.0 { "+" } else { "" };
            let init_quote_sign = if diff_from_initial_quote >= 0.0 { "+" } else { "" };
            println!("  累计变化: {}{:.8} {} | {}{:.4} {}",
                init_base_sign, diff_from_initial_base, base_asset,
                init_quote_sign, diff_from_initial_quote, quote_asset);

            // 检测异常：base 减少换算成 USDT > 1 或 quote 减少 > 1
            let base_loss_usdt = -diff_base * trade_price;
            let quote_loss = -diff_quote;
            if base_loss_usdt > 1.0 || quote_loss > 1.0 {
                println!("  ⚠️  警告: 本轮余额减少超过 1 USDT，可能被套利!");
            }

            let init_base_loss_usdt = -diff_from_initial_base * trade_price;
            let init_quote_loss = -diff_from_initial_quote;
            if init_base_loss_usdt > 1.0 || init_quote_loss > 1.0 {
                println!("  🚨 警告: 累计余额减少超过 1 USDT，策略可能被套利!");
            }

            // 更新上一轮余额
            prev_total_base = curr_total_base;
            prev_total_quote = curr_total_quote;

            // 本次执行成功，递增round
            round += 1;

            // 如果还有下一轮，等待间隔时间
            if round <= repeat {
                println!("\n⏳ 等待 {} 秒后进行第 {}/{} 次...", interval, round, repeat);
                tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
            }
        } // 内层循环结束

        // 如果还有下一轮toggle，切换方向并等待
        if toggle_round < toggle {
            current_direction = if current_direction == "buy" {
                "sell".to_string()
            } else {
                "buy".to_string()
            };
            println!("\n⏳ 等待 {} 秒后切换方向到 {}...", interval, current_direction);
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }
    } // 外层循环结束

    let total_executions = repeat * (toggle + 1);
    println!("\n{}", "═".repeat(60));
    println!("✅ 全部 {} 次对敲执行完毕！", total_executions);
    if toggle > 0 {
        println!("   (共 {} 轮方向切换，每轮 {} 次)", toggle + 1, repeat);
    }
    println!("{}", "═".repeat(60));

    Ok(())
}
