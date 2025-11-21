# Quick Start - Average Trading Price Feature

## 🚀 In 3 Steps

### Step 1: Rebuild the Project
```bash
# Backend (if you changed Rust code)
cargo build --release

# Frontend
cd frontend
npm run build
cd ..
```

### Step 2: Start the Report Server
```bash
./target/release/mexc-grid-trader report
```

### Step 3: View the Dashboard
Open your browser and go to: **http://localhost:3000**

Scroll down to the **"💹 Profit & Loss Summary"** section, and you'll see the new card:

```
┌─────────────────────────────────────────────────┐
│   📊 Average Trading Price Analysis             │
├─────────────────────────────────────────────────┤
│   Direction: Buying ZKWASM                      │
│   Average Price: 0.015050 USDT                  │
│                                                  │
│   [Calculation: 150.50 ÷ 10000.00000000]       │
│                                                  │
│   Current Market Price: 0.015200                │
│   Price Difference: -0.000150 (-0.99%)          │
│                                                  │
│   ✅ Bought below current market price         │
└─────────────────────────────────────────────────┘
```

## 📖 What Does It Show?

### Green (✅) = Good
- **When Buying**: You bought cheaper than current market price
- **When Selling**: You sold higher than current market price

### Orange (⚠️) = Warning
- **When Buying**: You bought more expensive than current market price
- **When Selling**: You sold lower than current market price

## 🧮 How It Calculates

```
Average Price = |USDT Change| ÷ |Token Change|
```

**Example:**
- Started with: 5000 USDT, 100000 ZKWASM
- Now have: 4849.50 USDT, 110000 ZKWASM
- USDT Change: -150.50 (spent)
- Token Change: +10000 (gained)
- **Average Buying Price = 150.50 ÷ 10000 = 0.015050 USDT**

## 🔍 When to Use

✅ **Good for:**
- Checking if your bot is getting good prices
- Validating your trading strategy
- Monitoring grid trading performance
- Understanding unrealized P&L

⚠️ **Be Careful:**
- Small trades (< $1) might show misleading data
- External deposits/withdrawals will affect the calculation
- Need at least 2 snapshots with balance changes

## 🎯 Real World Example

### Scenario: Grid Bot Running for 1 Hour

**Before:**
```
USDT: 1000.00
BTC: 1.50000000
```

**After:**
```
USDT: 925.75  (-74.25)
BTC: 1.55000000  (+0.05000000)
```

**Analysis:**
```
Average Buying Price = 74.25 ÷ 0.05 = 1485.00 USDT/BTC
Current Price: 1500.00 USDT/BTC
Difference: -15.00 (-1.00%)

✅ Bought below current market price!
Unrealized profit if sold now: 0.05 × 1500 - 74.25 = $0.75
```

## 💡 Pro Tips

1. **Watch the trend**: If consistently buying below market and selling above, your strategy is working!

2. **Compare with order distribution**: Check your "My Order Depth" to see if orders are well-positioned.

3. **Consider market volatility**: Large price differences might indicate fast-moving markets.

4. **Use with PnL summary**: The average price complements the total value change.

## ❓ FAQ

**Q: Why is the card not showing?**
A: You need at least 2 snapshots with different balances. Run the bot in watch mode for a while.

**Q: The price seems wrong?**
A: Check if you made any manual deposits or withdrawals. The calculation assumes all balance changes come from trading.

**Q: It shows a warning but I'm profitable?**
A: The warning only compares with current price. Check the "absolute_change" in PnL summary for actual profit/loss.

**Q: Can I see historical average prices?**
A: Not yet! This is planned for a future update.

## 📚 More Information

- **Full Documentation**: [AVERAGE_PRICE_FEATURE.md](AVERAGE_PRICE_FEATURE.md)
- **Dashboard Guide**: [REPORT_DASHBOARD.md](REPORT_DASHBOARD.md)
- **Changelog**: [CHANGELOG_AVERAGE_PRICE.md](CHANGELOG_AVERAGE_PRICE.md)

## 🐛 Issues?

If you encounter the "Invalid KuCoin response format" error:
1. Check your API credentials in config.json
2. Look for "DEBUG:" logs showing the actual API response
3. The error message now includes the response content for easier debugging

---

**Happy Trading! 🎉**
