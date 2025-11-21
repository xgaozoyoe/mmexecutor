# Average Trading Price Feature

## Overview
This feature automatically calculates the average price at which your trading bot executed trades during a reporting period, and compares it with the current market price to show trading performance.

## How It Works

### Calculation Method
```
Average Price = |Quote Asset Change| ÷ |Base Asset Change|
```

Where:
- **Quote Asset** = USDT (or other stable coin)
- **Base Asset** = The token being traded (e.g., ZKWASM, BTC, ETH)

### Example Scenarios

#### Scenario 1: Buying Tokens
```
Initial State:
  USDT: 5000.00
  ZKWASM: 100000.0

After Trading:
  USDT: 4849.50  (decreased by -150.50)
  ZKWASM: 110000.0  (increased by +10000.0)

Average Buying Price = 150.50 ÷ 10000.0 = 0.015050 USDT per ZKWASM
```

If current market price is 0.015200:
- Price Difference: -0.000150 USDT (-0.99%)
- **Result**: ✅ Bought below market price - Good trade!

#### Scenario 2: Selling Tokens
```
Initial State:
  USDT: 5000.00
  ZKWASM: 100000.0

After Trading:
  USDT: 5162.50  (increased by +162.50)
  ZKWASM: 90000.0  (decreased by -10000.0)

Average Selling Price = 162.50 ÷ 10000.0 = 0.016250 USDT per ZKWASM
```

If current market price is 0.015200:
- Price Difference: +0.001050 USDT (+6.91%)
- **Result**: ✅ Sold above market price - Excellent trade!

## UI Display

### Information Shown
1. **Direction**: Buying or Selling
2. **Average Price**: Calculated average execution price
3. **Calculation Details**: Shows the actual numbers used in calculation
4. **Current Market Price**: For real-time comparison
5. **Price Difference**: Both absolute and percentage
6. **Performance Indicator**: Visual feedback on trade quality

### Color Coding
- **Green** (✅): Good trading performance
  - When buying: Average price < Current price
  - When selling: Average price > Current price
- **Red/Orange** (⚠️): Less optimal trading
  - When buying: Average price > Current price
  - When selling: Average price < Current price

## Business Value

### Why This Matters
1. **Performance Monitoring**: Quickly see if your bot is getting good prices
2. **Strategy Validation**: Confirm your trading strategy is working as intended
3. **Profit Potential**: Understand unrealized gains/losses
4. **Market Timing**: See if you're buying low and selling high

### Use Cases
- **Grid Trading**: Verify the bot is capturing spread effectively
- **Market Making**: Ensure competitive price execution
- **Arbitrage**: Confirm profitable price differences
- **DCA Strategy**: Track average entry price over time

## Technical Implementation

### Frontend (React/TypeScript)
Located in: `frontend/src/App.tsx` (lines 345-394)

Key calculations:
```typescript
const avgPrice = Math.abs(quoteChange / baseChange);
const priceDiff = avgPrice - currentPrice;
const priceDiffPercent = (priceDiff / currentPrice) * 100;
```

### Backend (Rust)
The PnL summary data comes from: `src/api_server.rs`

Data flow:
1. Historical snapshots track account balances over time
2. PnL analyzer calculates changes between snapshots
3. Frontend receives changes for both assets
4. Frontend calculates average price from the changes

## Limitations & Considerations

### When Average Price is Meaningful
- ✅ During active trading periods
- ✅ When significant volume has been traded
- ✅ For analyzing completed trading cycles

### When to Be Cautious
- ⚠️ Very small balance changes (< $1)
- ⚠️ When no trades occurred (shows 0)
- ⚠️ During extreme market volatility
- ⚠️ When external deposits/withdrawals happened

### Accuracy Notes
- Prices are rounded to 6 decimal places for display
- Calculation uses the absolute values of changes
- Fees are implicitly included in the balance changes
- Assumes all balance changes came from trading

## Future Enhancements

Possible improvements:
1. **Trade History Integration**: Compare with actual fill prices
2. **Fee Breakdown**: Show separate fee impact
3. **Volume Weighted Average**: More sophisticated pricing
4. **Historical Chart**: Track average prices over multiple periods
5. **Benchmark Comparison**: Compare against TWAP/VWAP
6. **Alert System**: Notify when prices deviate significantly

## Screenshots & Examples

When viewing the dashboard, look for the **"📊 Average Trading Price Analysis"** card in the PnL section. It will display:

```
📊 Average Trading Price Analysis

Direction: Buying ZKWASM
Average Price: 0.015050 USDT
Calculation: 150.50 USDT ÷ 10000.00000000 ZKWASM

Current Market Price: 0.015200
Price Difference: -0.000150 (-0.99%)
✅ Bought below current market price
```

## Troubleshooting

### "No average price shown"
- No trades occurred during the period
- Both snapshots have identical balances
- PnL summary data is not available (need at least 2 snapshots)

### "Price seems incorrect"
- Check if external deposits/withdrawals occurred
- Verify snapshot data is from trading activity
- Consider market volatility during the period

### "Shows warning even though trading seems good"
- Market may have moved significantly since trades
- Consider unrealized P&L in the quote asset summary
- Check the percentage change in total portfolio value

## Related Documentation
- [REPORT_DASHBOARD.md](REPORT_DASHBOARD.md) - Full dashboard documentation
- [README.md](README.md) - General project documentation
- Backend API: `/api/report` endpoint structure
