use crate::account_snapshot::{AccountSnapshot, AssetSnapshot, PnLAnalyzer, SnapshotHistory};
use crate::config::Config;
use crate::exchange::Exchange;
use crate::exchanges::{create_exchange, ExchangeType};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    config_path: String,
    cache: Arc<RwLock<Option<ReportData>>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReportData {
    pub timestamp: String,
    pub exchanges: Vec<ExchangeReport>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExchangeReport {
    pub name: String,
    pub account_info: AccountInfo,
    pub market_depth: MarketDepth,
    pub order_depth: OrderDepth,
    pub snapshots_info: SnapshotsInfo,
    pub pnl_summary: Option<PnLSummary>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountInfo {
    pub base_asset: String,
    pub quote_asset: String,
    pub balances: Vec<AssetBalance>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetBalance {
    pub asset: String,
    pub free: f64,
    pub locked: f64,
    pub total: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketDepth {
    pub current_price: f64,
    pub best_bid: f64,
    pub best_ask: f64,
    pub depth_ranges: Vec<DepthRange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DepthRange {
    pub percentage: f64,
    pub bid_depth: f64,
    pub ask_depth: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderDepth {
    pub total_orders: usize,
    pub buy_orders: OrderSide,
    pub sell_orders: OrderSide,
    pub total_unfilled_value: f64,
    pub depth_by_percentage: Vec<OrderDepthRange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderSide {
    pub count: usize,
    pub price_min: Option<f64>,
    pub price_max: Option<f64>,
    pub total_qty: f64,
    pub total_value: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderDepthRange {
    pub percentage: f64,
    pub buy_count: usize,
    pub buy_value: f64,
    pub sell_count: usize,
    pub sell_value: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotsInfo {
    pub total_count: usize,
    pub latest: Option<SnapshotData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotData {
    pub datetime: String,
    pub iteration: Option<u64>,
    pub mid_price: Option<f64>,
    pub assets: Vec<AssetSnapshot>,
    pub total_value: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PnLSummary {
    pub period_start: String,
    pub period_end: String,
    pub duration_seconds: i64,
    pub quote_asset_summary: AssetSummary,
    pub base_asset_summary: AssetSummary,
    pub trade_stats: Option<TradeStatsSummary>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TradeStatsSummary {
    pub avg_buy_price: f64,
    pub avg_sell_price: f64,
    pub total_buy_quantity: f64,
    pub total_sell_quantity: f64,
    pub total_buy_cost: f64,
    pub total_sell_revenue: f64,
    pub buy_count: usize,
    pub sell_count: usize,
    pub net_quantity: f64,
    pub realized_pnl: f64,
    // 24小时统计
    pub buy_24h_quantity: f64,
    pub buy_24h_cost: f64,
    pub buy_24h_avg_price: f64,
    pub buy_24h_count: usize,
    pub sell_24h_quantity: f64,
    pub sell_24h_revenue: f64,
    pub sell_24h_avg_price: f64,
    pub sell_24h_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetSummary {
    pub asset: String,
    pub starting_value: f64,
    pub ending_value: f64,
    pub absolute_change: f64,
    pub percentage_change: f64,
}

// Custom error type for API responses
struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": self.0.to_string()
            })),
        )
            .into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

pub async fn start_server(config_path: String, port: u16) -> Result<()> {
    // Sync trade history at startup
    println!("📊 Syncing trade history...\n");
    if let Err(e) = sync_trade_history(&config_path).await {
        eprintln!("⚠️  Warning: Could not sync trade history: {}", e);
        eprintln!("Continuing anyway...\n");
    }

    let state = AppState {
        config_path: config_path.clone(),
        cache: Arc::new(RwLock::new(None)),
    };

    // Start background task to update cache and capture snapshots every 30 seconds
    let cache_state = state.clone();
    tokio::spawn(async move {
        let mut iteration_counter = 0u64;
        loop {
            iteration_counter += 1;
            match fetch_and_snapshot_report_data(&cache_state.config_path, Some(iteration_counter)).await {
                Ok(data) => {
                    let mut cache = cache_state.cache.write().await;
                    *cache = Some(data);
                    println!("✅ Report data updated at {}", chrono::Utc::now().format("%H:%M:%S UTC"));
                }
                Err(e) => {
                    eprintln!("❌ Failed to fetch report data: {}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    });

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/report", get(get_report))
        .route("/health", get(health_check))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("🚀 API Server starting on http://{}", addr);
    println!("📊 Report endpoint: http://localhost:{}/api/report", port);
    println!("💚 Health check: http://localhost:{}/health", port);
    println!("⚠️  Press Ctrl+C to stop");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn get_report(State(state): State<AppState>) -> Result<Json<ReportData>, ApiError> {
    let cache = state.cache.read().await;

    if let Some(data) = cache.as_ref() {
        Ok(Json(data.clone()))
    } else {
        // If cache is empty, fetch data immediately
        drop(cache);
        let data = fetch_report_data(&state.config_path).await?;
        let mut cache = state.cache.write().await;
        *cache = Some(data.clone());
        Ok(Json(data))
    }
}

async fn fetch_report_data(config_path: &str) -> Result<ReportData> {
    fetch_and_snapshot_report_data(config_path, None).await
}

async fn fetch_and_snapshot_report_data(config_path: &str, iteration: Option<u64>) -> Result<ReportData> {
    let config = Config::from_file(config_path)?;
    let mut exchanges = Vec::new();

    for exchange_config in &config.exchanges {
        match fetch_exchange_report_with_snapshot(exchange_config, config.grid.get_symbol(), iteration).await {
            Ok(report) => exchanges.push(report),
            Err(e) => {
                eprintln!("Warning: Failed to fetch report for {}: {}", exchange_config.name, e);
            }
        }
    }

    Ok(ReportData {
        timestamp: chrono::Utc::now().to_rfc3339(),
        exchanges,
    })
}

async fn fetch_exchange_report(
    exchange_config: &crate::config::ExchangeConfig,
    symbol: &str,
) -> Result<ExchangeReport> {
    fetch_exchange_report_with_snapshot(exchange_config, symbol, None).await
}

async fn fetch_exchange_report_with_snapshot(
    exchange_config: &crate::config::ExchangeConfig,
    symbol: &str,
    iteration: Option<u64>,
) -> Result<ExchangeReport> {
    let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_config.name))
        .context(format!("Invalid exchange type: {}", exchange_config.name))?;

    let client = create_exchange(
        &exchange_type,
        exchange_config.api_key.clone(),
        exchange_config.api_secret.clone(),
        exchange_config.api_passphrase.clone(),
    )?;

    let (base_asset, quote_asset) = client.get_symbol_assets(symbol);

    // Fetch account info
    let account_info = fetch_account_info(&client, &base_asset, &quote_asset).await?;

    // Fetch market depth
    let market_depth = fetch_market_depth(&client, symbol).await?;

    // Fetch order depth
    let order_depth = fetch_order_depth(&client, symbol).await?;

    // Fetch snapshots info
    let snapshots_info = fetch_snapshots_info(&exchange_config.name, symbol, &base_asset, &quote_asset)?;

    // Fetch PnL summary
    let pnl_summary = fetch_pnl_summary(&exchange_config.name, symbol, &base_asset, &quote_asset)?;

    // Capture and save snapshot if iteration is provided
    if iteration.is_some() {
        match capture_and_save_snapshot(
            &client,
            &exchange_config.name,
            symbol,
            Some(market_depth.current_price),
            iteration,
        ).await {
            Ok(_) => {
                // Snapshot saved successfully - silently continue
            }
            Err(e) => {
                // Log error but don't fail the entire report
                eprintln!("Warning: Failed to save snapshot for {}: {}", exchange_config.name, e);
            }
        }
    }

    Ok(ExchangeReport {
        name: exchange_config.name.clone(),
        account_info,
        market_depth,
        order_depth,
        snapshots_info,
        pnl_summary,
    })
}

async fn fetch_account_info(
    client: &Arc<dyn Exchange>,
    base_asset: &str,
    quote_asset: &str,
) -> Result<AccountInfo> {
    let account = client.get_account_info().await?;
    let mut balances = Vec::new();

    for balance in &account.balances {
        if balance.asset == base_asset || balance.asset == quote_asset {
            let free: f64 = balance.free.parse().unwrap_or(0.0);
            let locked: f64 = balance.locked.parse().unwrap_or(0.0);
            let total = free + locked;

            balances.push(AssetBalance {
                asset: balance.asset.clone(),
                free,
                locked,
                total,
            });
        }
    }

    Ok(AccountInfo {
        base_asset: base_asset.to_string(),
        quote_asset: quote_asset.to_string(),
        balances,
    })
}

async fn fetch_market_depth(client: &Arc<dyn Exchange>, symbol: &str) -> Result<MarketDepth> {
    let order_book = client.get_order_book(symbol, Some(50)).await?;

    if order_book.bids.is_empty() || order_book.asks.is_empty() {
        anyhow::bail!("Order book is empty");
    }

    let best_bid: f64 = order_book.bids[0][0].parse().context("Failed to parse best bid")?;
    let best_ask: f64 = order_book.asks[0][0].parse().context("Failed to parse best ask")?;
    let mid_price = (best_bid + best_ask) / 2.0;

    let percentages = vec![0.5, 1.0, 2.0, 5.0, 10.0];
    let mut depth_ranges = Vec::new();

    for pct in percentages {
        let bid_depth = calculate_depth_in_range(&order_book.bids, mid_price, pct, true)?;
        let ask_depth = calculate_depth_in_range(&order_book.asks, mid_price, pct, false)?;

        depth_ranges.push(DepthRange {
            percentage: pct,
            bid_depth,
            ask_depth,
        });
    }

    Ok(MarketDepth {
        current_price: mid_price,
        best_bid,
        best_ask,
        depth_ranges,
    })
}

fn calculate_depth_in_range(
    orders: &[Vec<String>],
    mid_price: f64,
    percentage: f64,
    is_bid: bool,
) -> Result<f64> {
    let mut total_depth = 0.0;

    let price_threshold = if is_bid {
        mid_price * (1.0 - percentage / 100.0)
    } else {
        mid_price * (1.0 + percentage / 100.0)
    };

    for order in orders {
        if order.len() < 2 {
            continue;
        }

        let price: f64 = order[0].parse().context("Failed to parse price")?;
        let quantity: f64 = order[1].parse().context("Failed to parse quantity")?;

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

async fn fetch_order_depth(client: &Arc<dyn Exchange>, symbol: &str) -> Result<OrderDepth> {
    let orders = client.get_open_orders(Some(symbol)).await?;

    if orders.is_empty() {
        return Ok(OrderDepth {
            total_orders: 0,
            buy_orders: OrderSide {
                count: 0,
                price_min: None,
                price_max: None,
                total_qty: 0.0,
                total_value: 0.0,
            },
            sell_orders: OrderSide {
                count: 0,
                price_min: None,
                price_max: None,
                total_qty: 0.0,
                total_value: 0.0,
            },
            total_unfilled_value: 0.0,
            depth_by_percentage: vec![],
        });
    }

    let buy_orders: Vec<_> = orders.iter().filter(|o| o.side == "BUY").collect();
    let sell_orders: Vec<_> = orders.iter().filter(|o| o.side == "SELL").collect();

    let buy_side = calculate_order_side(&buy_orders)?;
    let sell_side = calculate_order_side(&sell_orders)?;

    let total_unfilled_value = buy_side.total_value + sell_side.total_value;

    // Get market price for depth calculation
    let order_book = client.get_order_book(symbol, Some(10)).await?;
    let mid_price = if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
        let best_bid: f64 = order_book.bids[0][0].parse().unwrap_or(0.0);
        let best_ask: f64 = order_book.asks[0][0].parse().unwrap_or(0.0);
        (best_bid + best_ask) / 2.0
    } else {
        0.0
    };

    // Calculate depth by percentage ranges
    let percentages = vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0];
    let depth_by_percentage = if mid_price > 0.0 {
        calculate_order_depth_by_percentage(&orders, mid_price, &percentages)?
    } else {
        vec![]
    };

    Ok(OrderDepth {
        total_orders: orders.len(),
        buy_orders: buy_side,
        sell_orders: sell_side,
        total_unfilled_value,
        depth_by_percentage,
    })
}

fn calculate_order_side(orders: &[&crate::exchange::OpenOrder]) -> Result<OrderSide> {
    if orders.is_empty() {
        return Ok(OrderSide {
            count: 0,
            price_min: None,
            price_max: None,
            total_qty: 0.0,
            total_value: 0.0,
        });
    }

    let mut total_value = 0.0;
    let mut total_qty = 0.0;
    let mut prices = Vec::new();

    for order in orders {
        let price: f64 = order.price.parse().unwrap_or(0.0);
        let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
        let filled: f64 = order.executed_qty.parse().unwrap_or(0.0);
        let remaining_qty = qty - filled;

        total_value += price * remaining_qty;
        total_qty += remaining_qty;
        prices.push(price);
    }

    let price_min = prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let price_max = prices.iter().fold(0.0f64, |a, &b| a.max(b));

    Ok(OrderSide {
        count: orders.len(),
        price_min: Some(price_min),
        price_max: Some(price_max),
        total_qty,
        total_value,
    })
}

fn calculate_order_depth_by_percentage(
    orders: &[crate::exchange::OpenOrder],
    mid_price: f64,
    percentages: &[f64],
) -> Result<Vec<OrderDepthRange>> {
    let mut result = Vec::new();

    for &pct in percentages {
        let price_threshold_low = mid_price * (1.0 - pct / 100.0);
        let price_threshold_high = mid_price * (1.0 + pct / 100.0);

        let mut buy_count = 0;
        let mut buy_value = 0.0;
        let mut sell_count = 0;
        let mut sell_value = 0.0;

        for order in orders {
            let price: f64 = order.price.parse().unwrap_or(0.0);
            let qty: f64 = order.orig_qty.parse().unwrap_or(0.0);
            let filled: f64 = order.executed_qty.parse().unwrap_or(0.0);
            let remaining_qty = qty - filled;
            let value = price * remaining_qty;

            if order.side == "BUY" {
                // Buy orders: within range from mid_price down to threshold
                if price >= price_threshold_low && price <= mid_price {
                    buy_count += 1;
                    buy_value += value;
                }
            } else if order.side == "SELL" {
                // Sell orders: within range from mid_price up to threshold
                if price >= mid_price && price <= price_threshold_high {
                    sell_count += 1;
                    sell_value += value;
                }
            }
        }

        result.push(OrderDepthRange {
            percentage: pct,
            buy_count,
            buy_value,
            sell_count,
            sell_value,
        });
    }

    Ok(result)
}

fn fetch_snapshots_info(
    exchange: &str,
    symbol: &str,
    base_asset: &str,
    quote_asset: &str,
) -> Result<SnapshotsInfo> {
    let history = SnapshotHistory::new(exchange, symbol);
    let snapshots = history.load_all()?;

    let latest = snapshots.last().map(|s| {
        let total_value = s.calculate_total_value(base_asset, quote_asset);
        SnapshotData {
            datetime: s.datetime.clone(),
            iteration: s.iteration,
            mid_price: s.mid_price,
            assets: s.assets.clone(),
            total_value,
        }
    });

    Ok(SnapshotsInfo {
        total_count: snapshots.len(),
        latest,
    })
}

async fn capture_and_save_snapshot(
    client: &Arc<dyn Exchange>,
    exchange: &str,
    symbol: &str,
    mid_price: Option<f64>,
    iteration: Option<u64>,
) -> Result<()> {
    // Get account info
    let account = client.get_account_info().await?;
    let (base_asset, quote_asset) = client.get_symbol_assets(symbol);

    // Build asset snapshots
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

    // Create snapshot
    let snapshot = AccountSnapshot::new(exchange, symbol, assets, mid_price, iteration);

    // Save to history
    let history = SnapshotHistory::new(exchange, symbol);
    history.append_snapshot(&snapshot)?;

    Ok(())
}

fn fetch_pnl_summary(
    exchange: &str,
    symbol: &str,
    base_asset: &str,
    quote_asset: &str,
) -> Result<Option<PnLSummary>> {
    use crate::trade_history::TradeHistory;
    use crate::trade_analysis::TradingStats;

    let history = SnapshotHistory::new(exchange, symbol);
    let snapshots = history.load_all()?;

    if snapshots.len() < 2 {
        return Ok(None);
    }

    if let Some(analysis) = PnLAnalyzer::analyze_period(&snapshots, base_asset, quote_asset) {
        // Use overall_change which is a PnLReport
        let report = &analysis.overall_change;

        // Get asset changes for both base and quote assets
        let earlier = &report.earlier_snapshot;
        let later = &report.later_snapshot;

        // Calculate quote asset (USDT) summary
        let quote_earlier = earlier.assets.iter()
            .find(|a| a.asset == quote_asset)
            .map(|a| a.total)
            .unwrap_or(0.0);
        let quote_later = later.assets.iter()
            .find(|a| a.asset == quote_asset)
            .map(|a| a.total)
            .unwrap_or(0.0);
        let quote_change = quote_later - quote_earlier;
        let quote_pct = if quote_earlier > 0.0 {
            (quote_change / quote_earlier) * 100.0
        } else {
            0.0
        };

        // Calculate base asset summary
        let base_earlier = earlier.assets.iter()
            .find(|a| a.asset == base_asset)
            .map(|a| a.total)
            .unwrap_or(0.0);
        let base_later = later.assets.iter()
            .find(|a| a.asset == base_asset)
            .map(|a| a.total)
            .unwrap_or(0.0);
        let base_change = base_later - base_earlier;
        let base_pct = if base_earlier > 0.0 {
            (base_change / base_earlier) * 100.0
        } else {
            0.0
        };

        // Load trade history and calculate statistics
        let trade_stats = {
            let trade_history = TradeHistory::new(exchange, symbol);
            match trade_history.load_all() {
                Ok(trades) if !trades.is_empty() => {
                    // Filter trades within the snapshot period
                    let start_ts = earlier.timestamp * 1000; // Convert to milliseconds
                    let end_ts = later.timestamp * 1000;
                    let period_trades: Vec<_> = trades
                        .into_iter()
                        .filter(|t| t.time >= start_ts && t.time <= end_ts)
                        .collect();

                    if !period_trades.is_empty() {
                        match TradingStats::from_trades(&period_trades, symbol, base_asset, quote_asset) {
                            Ok(stats) => Some(TradeStatsSummary {
                                avg_buy_price: stats.avg_buy_price,
                                avg_sell_price: stats.avg_sell_price,
                                total_buy_quantity: stats.total_buy_quantity,
                                total_sell_quantity: stats.total_sell_quantity,
                                total_buy_cost: stats.total_buy_cost,
                                total_sell_revenue: stats.total_sell_revenue,
                                buy_count: stats.buy_count,
                                sell_count: stats.sell_count,
                                net_quantity: stats.net_quantity,
                                realized_pnl: stats.realized_pnl,
                                buy_24h_quantity: stats.buy_24h_quantity,
                                buy_24h_cost: stats.buy_24h_cost,
                                buy_24h_avg_price: stats.buy_24h_avg_price,
                                buy_24h_count: stats.buy_24h_count,
                                sell_24h_quantity: stats.sell_24h_quantity,
                                sell_24h_revenue: stats.sell_24h_revenue,
                                sell_24h_avg_price: stats.sell_24h_avg_price,
                                sell_24h_count: stats.sell_24h_count,
                            }),
                            Err(_) => None,
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };

        let summary = PnLSummary {
            period_start: report.earlier_snapshot.datetime.clone(),
            period_end: report.later_snapshot.datetime.clone(),
            duration_seconds: report.time_elapsed_seconds,
            quote_asset_summary: AssetSummary {
                asset: quote_asset.to_string(),
                starting_value: quote_earlier,
                ending_value: quote_later,
                absolute_change: quote_change,
                percentage_change: quote_pct,
            },
            base_asset_summary: AssetSummary {
                asset: base_asset.to_string(),
                starting_value: base_earlier,
                ending_value: base_later,
                absolute_change: base_change,
                percentage_change: base_pct,
            },
            trade_stats,
        };
        Ok(Some(summary))
    } else {
        Ok(None)
    }
}

/// 验证和对账调整记录
// DISABLED: adjustment module not yet implemented
#[allow(dead_code)]
async fn verify_adjustments(_config_path: &str) -> Result<()> {
    anyhow::bail!("Adjustment verification not yet implemented")
}

/// 通过API验证调整记录（可选）
// DISABLED: adjustment module not yet implemented
#[allow(dead_code)]
async fn verify_adjustments_with_api(
    _exchange_config: &crate::config::ExchangeConfig,
    _grid_config: &crate::config::GridConfig,
    _adjustments: &[()],  // Changed from Adjustment to () to avoid compilation error
) -> Result<String> {
    anyhow::bail!("Adjustment verification not yet implemented")
}

/// 同步交易历史（从上一个 snapshot 到现在的所有交易）
async fn sync_trade_history(config_path: &str) -> Result<()> {
    use crate::trade_history::TradeHistory;
    use crate::account_snapshot::SnapshotHistory;

    let config = Config::from_file(config_path)?;

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║         SYNCING TRADE HISTORY FROM API                ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    for exchange_config in &config.exchanges {
        let exchange_name = &exchange_config.name;
        println!("📊 Processing {} ...", exchange_name);
        println!("{}", "-".repeat(60));

        // 创建交易所客户端
        let exchange_type: ExchangeType = serde_json::from_str(&format!("\"{}\"", exchange_name))
            .context(format!("Invalid exchange type: {}", exchange_name))?;

        let client = create_exchange(
            &exchange_type,
            exchange_config.api_key.clone(),
            exchange_config.api_secret.clone(),
            exchange_config.api_passphrase.clone(),
        )?;

        let symbol = config.grid.get_symbol();
        let trade_history = TradeHistory::new(exchange_name, symbol);
        let snapshot_history = SnapshotHistory::new(exchange_name, symbol);

        // 获取上一次同步的时间点
        let last_sync_time = match trade_history.get_last_trade_time()? {
            Some(t) => {
                println!("  📅 Last synced trade: {}",
                    chrono::DateTime::from_timestamp(t / 1000, 0)
                        .unwrap_or_default()
                        .format("%Y-%m-%d %H:%M:%S UTC"));
                t
            }
            None => {
                // 如果没有交易记录，从第一个 snapshot 开始
                let snapshots = snapshot_history.load_all()?;
                if let Some(first) = snapshots.first() {
                    let ts = first.timestamp * 1000;
                    println!("  📅 No trade history found, syncing from first snapshot: {}", first.datetime);
                    ts
                } else {
                    // 如果没有 snapshot，从 30 天前开始
                    let ts = (chrono::Utc::now() - chrono::Duration::days(30)).timestamp() * 1000;
                    println!("  📅 No snapshots found, syncing from 30 days ago");
                    ts
                }
            }
        };

        // 查询交易历史
        println!("  ⏰ Fetching trades from {} to now...",
            chrono::DateTime::from_timestamp(last_sync_time / 1000, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S UTC"));

        match client.get_my_trades(symbol, Some(1000)).await {
            Ok(trades) => {
                // 过滤出时间范围内的交易
                let new_trades: Vec<_> = trades
                    .into_iter()
                    .filter(|t| t.time > last_sync_time)
                    .collect();

                if new_trades.is_empty() {
                    println!("  ✅ No new trades found\n");
                } else {
                    // 去重并保存
                    let added = trade_history.add_trades_deduplicated(&new_trades)?;
                    println!("  ✅ Added {} new trade(s) (total fetched: {})\n",
                        added, new_trades.len());
                }
            }
            Err(e) => {
                eprintln!("  ⚠️  Could not fetch trades: {}\n", e);
            }
        }
    }

    println!("{}", "═".repeat(60));
    println!("✅ Trade history sync complete!");
    println!("{}", "═".repeat(60));
    println!();

    Ok(())
}
