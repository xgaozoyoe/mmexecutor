import React, { useEffect, useState } from 'react';
import './App.css';

interface AssetBalance {
  asset: string;
  free: number;
  locked: number;
  total: number;
}

interface AccountInfo {
  base_asset: string;
  quote_asset: string;
  balances: AssetBalance[];
}

interface DepthRange {
  percentage: number;
  bid_depth: number;
  ask_depth: number;
}

interface MarketDepth {
  current_price: number;
  best_bid: number;
  best_ask: number;
  depth_ranges: DepthRange[];
}

interface OrderSide {
  count: number;
  price_min: number | null;
  price_max: number | null;
  total_qty: number;
  total_value: number;
}

interface OrderDepth {
  total_orders: number;
  buy_orders: OrderSide;
  sell_orders: OrderSide;
  total_unfilled_value: number;
  depth_by_percentage: OrderDepthRange[];
}

interface OrderDepthRange {
  percentage: number;
  buy_count: number;
  buy_value: number;
  sell_count: number;
  sell_value: number;
}

interface AssetSnapshot {
  asset: string;
  free: number;
  locked: number;
  total: number;
}

interface SnapshotData {
  datetime: string;
  iteration: number | null;
  mid_price: number | null;
  assets: AssetSnapshot[];
  total_value: number | null;
}

interface SnapshotsInfo {
  total_count: number;
  latest: SnapshotData | null;
}

interface PnLSummary {
  period_start: string;
  period_end: string;
  duration_seconds: number;
  quote_asset_summary: AssetSummary;
  base_asset_summary: AssetSummary;
  trade_stats: TradeStatsSummary | null;
}

interface AssetSummary {
  asset: string;
  starting_value: number;
  ending_value: number;
  absolute_change: number;
  percentage_change: number;
}

interface TradeStatsSummary {
  avg_buy_price: number;
  avg_sell_price: number;
  total_buy_quantity: number;
  total_sell_quantity: number;
  total_buy_cost: number;
  total_sell_revenue: number;
  buy_count: number;
  sell_count: number;
  net_quantity: number;
  realized_pnl: number;
}

interface ExchangeReport {
  name: string;
  account_info: AccountInfo;
  market_depth: MarketDepth;
  order_depth: OrderDepth;
  snapshots_info: SnapshotsInfo;
  pnl_summary: PnLSummary | null;
}

interface ReportData {
  timestamp: string;
  exchanges: ExchangeReport[];
}

function App() {
  const [reportData, setReportData] = useState<ReportData | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null);
  const [activeTab, setActiveTab] = useState<number>(0);

  const apiUrl = process.env.REACT_APP_API_URL || 'http://localhost:3000';

  const fetchReport = async () => {
    try {
      const response = await fetch(`${apiUrl}/api/report`);
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const data: ReportData = await response.json();
      setReportData(data);
      setLastUpdate(new Date());
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch report');
      console.error('Error fetching report:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchReport();
    const interval = setInterval(fetchReport, 30000); // Refresh every 30 seconds
    return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (loading && !reportData) {
    return (
      <div className="App">
        <div className="loading">Loading report data...</div>
      </div>
    );
  }

  if (error && !reportData) {
    return (
      <div className="App">
        <div className="error">Error: {error}</div>
      </div>
    );
  }

  return (
    <div className="App">
      <header className="App-header">
        <h1>🤖 Trading Bot Report Dashboard</h1>
        <div className="last-update">
          Last Update: {lastUpdate?.toLocaleString() || 'Never'}
          {loading && <span className="loading-indicator"> ⟳ Refreshing...</span>}
        </div>
      </header>

      {/* Exchange Tabs */}
      {reportData && reportData.exchanges.length > 1 && (
        <div className="tabs-container">
          <div className="tabs">
            {reportData.exchanges.map((exchange, index) => (
              <button
                key={exchange.name}
                className={`tab ${activeTab === index ? 'active' : ''}`}
                onClick={() => setActiveTab(index)}
              >
                {exchange.name}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Active Exchange Content */}
      {reportData?.exchanges.map((exchange, index) => (
        <div
          key={exchange.name}
          className="exchange-section"
          style={{ display: activeTab === index ? 'block' : 'none' }}
        >
          <h2 className="exchange-name">📊 {exchange.name}</h2>

          {/* Account Info */}
          <section className="card">
            <h3>💰 Account Balance</h3>
            <div className="balance-grid">
              {exchange.account_info.balances.map((balance) => (
                <div key={balance.asset} className="balance-item">
                  <div className="balance-asset">{balance.asset}</div>
                  <div className="balance-details">
                    <div>Free: <strong>{balance.free.toFixed(8)}</strong></div>
                    <div>Locked: <strong>{balance.locked.toFixed(8)}</strong></div>
                    <div>Total: <strong>{balance.total.toFixed(8)}</strong></div>
                  </div>
                </div>
              ))}
            </div>
          </section>

          {/* Market Depth */}
          <section className="card">
            <h3>📈 Market Depth</h3>
            <div className="market-price">
              <div>Current Price: <strong>{exchange.market_depth.current_price.toFixed(6)}</strong></div>
              <div>Best Bid: <strong>{exchange.market_depth.best_bid.toFixed(6)}</strong></div>
              <div>Best Ask: <strong>{exchange.market_depth.best_ask.toFixed(6)}</strong></div>
            </div>
            <table className="depth-table">
              <thead>
                <tr>
                  <th>Range</th>
                  <th>Bid Depth (USDT)</th>
                  <th>Ask Depth (USDT)</th>
                </tr>
              </thead>
              <tbody>
                {exchange.market_depth.depth_ranges.map((range) => (
                  <tr key={range.percentage}>
                    <td>±{range.percentage}%</td>
                    <td>{range.bid_depth.toFixed(2)}</td>
                    <td>{range.ask_depth.toFixed(2)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>

          {/* Order Depth */}
          <section className="card">
            <h3>📋 My Order Depth</h3>
            <div className="order-summary">
              <div>Total Orders: <strong>{exchange.order_depth.total_orders}</strong></div>
            </div>
            <div className="order-sides">
              <div className="order-side buy">
                <h4>🟢 BUY Orders</h4>
                <div>Count: <strong>{exchange.order_depth.buy_orders.count}</strong></div>
                {exchange.order_depth.buy_orders.price_min && (
                  <div>
                    Price Range: <strong>
                      {exchange.order_depth.buy_orders.price_min.toFixed(6)} - {exchange.order_depth.buy_orders.price_max?.toFixed(6)}
                    </strong>
                  </div>
                )}
                <div>Total Qty: <strong>{exchange.order_depth.buy_orders.total_qty.toFixed(5)}</strong></div>
                <div>Total Value: <strong>{exchange.order_depth.buy_orders.total_value.toFixed(2)} USDT</strong></div>
              </div>
              <div className="order-side sell">
                <h4>🔴 SELL Orders</h4>
                <div>Count: <strong>{exchange.order_depth.sell_orders.count}</strong></div>
                {exchange.order_depth.sell_orders.price_min && (
                  <div>
                    Price Range: <strong>
                      {exchange.order_depth.sell_orders.price_min.toFixed(6)} - {exchange.order_depth.sell_orders.price_max?.toFixed(6)}
                    </strong>
                  </div>
                )}
                <div>Total Qty: <strong>{exchange.order_depth.sell_orders.total_qty.toFixed(5)}</strong></div>
                <div>Total Value: <strong>{exchange.order_depth.sell_orders.total_value.toFixed(2)} USDT</strong></div>
              </div>
            </div>
            <div className="total-unfilled">
              Total Unfilled Value: <strong>{exchange.order_depth.total_unfilled_value.toFixed(2)} USDT</strong>
            </div>

            {/* Depth by Percentage */}
            {exchange.order_depth.depth_by_percentage.length > 0 && (
              <div className="depth-by-percentage">
                <h4>📊 Order Depth by Price Range</h4>
                <table className="depth-table">
                  <thead>
                    <tr>
                      <th>Range</th>
                      <th>Buy Orders</th>
                      <th>Buy Value (USDT)</th>
                      <th>Sell Orders</th>
                      <th>Sell Value (USDT)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {exchange.order_depth.depth_by_percentage.map((range) => (
                      <tr key={range.percentage}>
                        <td>±{range.percentage}%</td>
                        <td>{range.buy_count}</td>
                        <td>{range.buy_value.toFixed(2)}</td>
                        <td>{range.sell_count}</td>
                        <td>{range.sell_value.toFixed(2)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {/* Snapshots Info */}
          <section className="card">
            <h3>📸 Snapshots History</h3>
            <div>Total Snapshots: <strong>{exchange.snapshots_info.total_count}</strong></div>
            {exchange.snapshots_info.latest && (
              <div className="latest-snapshot">
                <h4>Latest Snapshot</h4>
                <div>Datetime: <strong>{exchange.snapshots_info.latest.datetime}</strong></div>
                {exchange.snapshots_info.latest.iteration && (
                  <div>Iteration: <strong>#{exchange.snapshots_info.latest.iteration}</strong></div>
                )}
                {exchange.snapshots_info.latest.mid_price && (
                  <div>Mid Price: <strong>{exchange.snapshots_info.latest.mid_price.toFixed(6)}</strong></div>
                )}
                {exchange.snapshots_info.latest.total_value && (
                  <div>Total Value: <strong>{exchange.snapshots_info.latest.total_value.toFixed(2)} {exchange.account_info.quote_asset}</strong></div>
                )}
              </div>
            )}
          </section>

          {/* PnL Summary */}
          {exchange.pnl_summary && (
            <section className="card pnl-card">
              <h3>💹 Profit & Loss Summary</h3>
              <div className="pnl-period">
                <div>Period Start: <strong>{new Date(exchange.pnl_summary.period_start).toLocaleString()}</strong></div>
                <div>Period End: <strong>{new Date(exchange.pnl_summary.period_end).toLocaleString()}</strong></div>
                <div>Duration: <strong>{(exchange.pnl_summary.duration_seconds / 60).toFixed(1)} minutes</strong></div>
              </div>

              <div className="pnl-assets">
                {/* Quote Asset (USDT) Summary */}
                <div className="pnl-asset-card">
                  <h4>💵 {exchange.pnl_summary.quote_asset_summary.asset}</h4>
                  <div className="pnl-details">
                    <div>Starting: <strong>{exchange.pnl_summary.quote_asset_summary.starting_value.toFixed(2)}</strong></div>
                    <div>Ending: <strong>{exchange.pnl_summary.quote_asset_summary.ending_value.toFixed(2)}</strong></div>
                    <div className={exchange.pnl_summary.quote_asset_summary.absolute_change >= 0 ? 'positive' : 'negative'}>
                      Change: <strong>
                        {exchange.pnl_summary.quote_asset_summary.absolute_change >= 0 ? '+' : ''}
                        {exchange.pnl_summary.quote_asset_summary.absolute_change.toFixed(2)}
                        ({exchange.pnl_summary.quote_asset_summary.percentage_change >= 0 ? '+' : ''}
                        {exchange.pnl_summary.quote_asset_summary.percentage_change.toFixed(3)}%)
                      </strong>
                    </div>
                  </div>
                </div>

                {/* Base Asset Summary */}
                <div className="pnl-asset-card">
                  <h4>🪙 {exchange.pnl_summary.base_asset_summary.asset}</h4>
                  <div className="pnl-details">
                    <div>Starting: <strong>{exchange.pnl_summary.base_asset_summary.starting_value.toFixed(8)}</strong></div>
                    <div>Ending: <strong>{exchange.pnl_summary.base_asset_summary.ending_value.toFixed(8)}</strong></div>
                    <div className={exchange.pnl_summary.base_asset_summary.absolute_change >= 0 ? 'positive' : 'negative'}>
                      Change: <strong>
                        {exchange.pnl_summary.base_asset_summary.absolute_change >= 0 ? '+' : ''}
                        {exchange.pnl_summary.base_asset_summary.absolute_change.toFixed(8)}
                        ({exchange.pnl_summary.base_asset_summary.percentage_change >= 0 ? '+' : ''}
                        {exchange.pnl_summary.base_asset_summary.percentage_change.toFixed(3)}%)
                      </strong>
                    </div>
                  </div>
                </div>

                {/* Average Trading Price - From Trade History */}
                {exchange.pnl_summary.trade_stats && (
                  <div className="pnl-asset-card pnl-avg-price">
                    <h4>📊 Trading Statistics (from actual trades)</h4>
                    <div className="pnl-details">
                      {/* Buy Statistics */}
                      {exchange.pnl_summary.trade_stats.buy_count > 0 && (
                        <div className="trade-direction-section">
                          <h5>🟢 Buy Trades</h5>
                          <div>Orders: <strong>{exchange.pnl_summary.trade_stats.buy_count}</strong></div>
                          <div>Total Bought: <strong>{exchange.pnl_summary.trade_stats.total_buy_quantity.toFixed(8)} {exchange.pnl_summary.base_asset_summary.asset}</strong></div>
                          <div>Total Cost: <strong>{exchange.pnl_summary.trade_stats.total_buy_cost.toFixed(2)} {exchange.pnl_summary.quote_asset_summary.asset}</strong></div>
                          <div>Avg Buy Price: <strong className="highlight-price">{exchange.pnl_summary.trade_stats.avg_buy_price.toFixed(6)} {exchange.pnl_summary.quote_asset_summary.asset}</strong></div>
                          {(() => {
                            const currentPrice = exchange.market_depth.current_price;
                            const buyPriceDiff = exchange.pnl_summary.trade_stats.avg_buy_price - currentPrice;
                            const buyPriceDiffPercent = currentPrice > 0 ? (buyPriceDiff / currentPrice) * 100 : 0;
                            return (
                              <div className={buyPriceDiff < 0 ? 'price-diff-positive' : 'price-diff-negative'}>
                                vs Market: <strong>{buyPriceDiff >= 0 ? '+' : ''}{buyPriceDiff.toFixed(6)} ({buyPriceDiff >= 0 ? '+' : ''}{buyPriceDiffPercent.toFixed(2)}%)</strong>
                                <div className="performance-indicator">
                                  {buyPriceDiff < 0 ? '✅ Bought below current market price' : '⚠️ Bought above current market price'}
                                </div>
                              </div>
                            );
                          })()}
                        </div>
                      )}

                      {/* Sell Statistics */}
                      {exchange.pnl_summary.trade_stats.sell_count > 0 && (
                        <div className="trade-direction-section">
                          <h5>🔴 Sell Trades</h5>
                          <div>Orders: <strong>{exchange.pnl_summary.trade_stats.sell_count}</strong></div>
                          <div>Total Sold: <strong>{exchange.pnl_summary.trade_stats.total_sell_quantity.toFixed(8)} {exchange.pnl_summary.base_asset_summary.asset}</strong></div>
                          <div>Total Revenue: <strong>{exchange.pnl_summary.trade_stats.total_sell_revenue.toFixed(2)} {exchange.pnl_summary.quote_asset_summary.asset}</strong></div>
                          <div>Avg Sell Price: <strong className="highlight-price">{exchange.pnl_summary.trade_stats.avg_sell_price.toFixed(6)} {exchange.pnl_summary.quote_asset_summary.asset}</strong></div>
                          {(() => {
                            const currentPrice = exchange.market_depth.current_price;
                            const sellPriceDiff = exchange.pnl_summary.trade_stats.avg_sell_price - currentPrice;
                            const sellPriceDiffPercent = currentPrice > 0 ? (sellPriceDiff / currentPrice) * 100 : 0;
                            return (
                              <div className={sellPriceDiff > 0 ? 'price-diff-positive' : 'price-diff-negative'}>
                                vs Market: <strong>{sellPriceDiff >= 0 ? '+' : ''}{sellPriceDiff.toFixed(6)} ({sellPriceDiff >= 0 ? '+' : ''}{sellPriceDiffPercent.toFixed(2)}%)</strong>
                                <div className="performance-indicator">
                                  {sellPriceDiff > 0 ? '✅ Sold above current market price' : '⚠️ Sold below current market price'}
                                </div>
                              </div>
                            );
                          })()}
                        </div>
                      )}

                      {/* Current Market Price */}
                      <div className="market-price-section">
                        <div>Current Market Price: <strong>{exchange.market_depth.current_price.toFixed(6)}</strong></div>
                      </div>

                      {/* Spread and PnL */}
                      {exchange.pnl_summary.trade_stats.buy_count > 0 && exchange.pnl_summary.trade_stats.sell_count > 0 && (
                        <div className="spread-section">
                          <div>Avg Spread: <strong>
                            {(exchange.pnl_summary.trade_stats.avg_sell_price - exchange.pnl_summary.trade_stats.avg_buy_price).toFixed(6)}
                            ({((exchange.pnl_summary.trade_stats.avg_sell_price - exchange.pnl_summary.trade_stats.avg_buy_price) / exchange.pnl_summary.trade_stats.avg_buy_price * 100).toFixed(2)}%)
                          </strong></div>
                          <div className={exchange.pnl_summary.trade_stats.realized_pnl >= 0 ? 'positive' : 'negative'}>
                            Realized PnL: <strong>
                              {exchange.pnl_summary.trade_stats.realized_pnl >= 0 ? '+' : ''}
                              {exchange.pnl_summary.trade_stats.realized_pnl.toFixed(2)} {exchange.pnl_summary.quote_asset_summary.asset}
                            </strong>
                          </div>
                        </div>
                      )}

                      {/* Net Position */}
                      <div className="net-position-section">
                        <div>Net Position: <strong>
                          {exchange.pnl_summary.trade_stats.net_quantity >= 0 ? '+' : ''}
                          {exchange.pnl_summary.trade_stats.net_quantity.toFixed(8)} {exchange.pnl_summary.base_asset_summary.asset}
                        </strong></div>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </section>
          )}
        </div>
      ))}

      {error && (
        <div className="error-banner">
          ⚠️ {error}
        </div>
      )}
    </div>
  );
}

export default App;
