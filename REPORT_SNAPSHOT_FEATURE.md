# Report Command: Automatic Snapshot Generation

## 🎯 What's New

The `report` command now **automatically generates snapshots**, just like the `watch` command!

## ✨ Benefits

### Before (Old Way)
```
Server: watch command → generates snapshots
Local:  report command → only shows API data (no PnL, no average price)
```
❌ Problem: You had to run `watch` on your local machine or manually copy snapshot files

### After (New Way)
```
Server: watch command → generates snapshots (for server monitoring)
Local:  report command → generates its OWN snapshots (independent data)
```
✅ Solution: Each environment collects its own data independently!

## 🚀 Usage

### Simply start the report server:
```bash
./target/release/mexc-grid-trader report --config config.json
```

That's it! The server will automatically:
1. ✅ Fetch and display real-time data
2. ✅ Capture account snapshots every 30 seconds
3. ✅ Calculate PnL after 2+ snapshots
4. ✅ Show average trading prices
5. ✅ Store data locally in `.snapshots_*.jsonl` files

## 📊 Timeline

```
Time    | Action
--------|--------------------------------------------------------
0:00    | Start report server
0:30    | 1st snapshot saved → No PnL yet (need 2 snapshots)
1:00    | 2nd snapshot saved → ✅ PnL Summary appears!
1:30    | 3rd snapshot saved → ✅ Average Price shows!
2:00    | 4th snapshot saved → More accurate analysis
...     | Continuous updates every 30 seconds
```

## 💾 Data Storage

### Snapshot Files
```
.snapshots_mexc_ZKWASMUSDT.jsonl
.snapshots_gate_ZKWASMUSDT.jsonl
.snapshots_kucoin_ZKWASMUSDT.jsonl
```

Each file contains JSON lines with account snapshots:
```json
{"datetime":"2025-11-21 15:30:00 UTC","iteration":1,"exchange":"gate","symbol":"ZKWASMUSDT","mid_price":0.015200,"assets":[...]}
{"datetime":"2025-11-21 15:30:30 UTC","iteration":2,"exchange":"gate","symbol":"ZKWASMUSDT","mid_price":0.015180,"assets":[...]}
```

### Important Notes
- ✅ Each exchange has its own snapshot file
- ✅ Server and local snapshots are completely separate (different files)
- ✅ Snapshots persist across restarts
- ✅ No file size limit issues (JSONL format is efficient)

## 🔍 Checking if It's Working

### Method 1: Watch the Console
```bash
./target/release/mexc-grid-trader report
```

You should see:
```
🚀 API Server starting on http://0.0.0.0:3000
📊 Report endpoint: http://localhost:3000/api/report
💚 Health check: http://localhost:3000/health
⚠️  Press Ctrl+C to stop

✅ Report data updated at 15:30:00 UTC
✅ Report data updated at 15:30:30 UTC
✅ Report data updated at 15:31:00 UTC
```

### Method 2: Check Snapshot Files
```bash
ls -lh .snapshots_*

# View latest snapshot
tail -1 .snapshots_gate_ZKWASMUSDT.jsonl | python3 -m json.tool
```

### Method 3: Use the Test Script
```bash
./test_snapshot_generation.sh
```

This will:
1. Start the report server
2. Wait for snapshots to be generated
3. Verify snapshot files exist
4. Check API response
5. Clean up

## 🎨 What You'll See in the Dashboard

After 2+ snapshots:

### PnL Summary Card
```
💹 Profit & Loss Summary
Period Start: 2025-11-21 15:30:00
Period End: 2025-11-21 16:00:00
Duration: 30.0 minutes

💵 USDT
  Starting: 5000.00
  Ending: 4850.00
  Change: -150.00 (-3.00%)

🪙 ZKWASM
  Starting: 100000.00000000
  Ending: 110000.00000000
  Change: +10000.00000000 (+10.00%)
```

### Average Trading Price Card
```
📊 Average Trading Price Analysis
Direction: Buying ZKWASM
Average Price: 0.015000 USDT

Calculation: 150.00 USDT ÷ 10000.00000000 ZKWASM

Current Market Price: 0.015200
Price Difference: -0.000200 (-1.32%)

✅ Bought below current market price
```

## 🔧 Configuration

### Snapshot Interval
Currently fixed at 30 seconds. To change, modify `api_server.rs`:
```rust
tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
// Change to 60 for 1-minute intervals
```

### Data Location
Snapshots are saved in the same directory as the executable. To change location, modify the `SnapshotHistory` implementation in `account_snapshot.rs`.

## 🆚 Watch vs Report

| Feature | Watch Command | Report Command |
|---------|--------------|----------------|
| Purpose | Automated trading + monitoring | Dashboard + monitoring |
| Snapshots | ✅ Yes | ✅ Yes (NEW!) |
| Data Location | `.snapshots_*` | `.snapshots_*` (separate files) |
| UI | Terminal output | Web dashboard |
| API Server | No | Yes (port 3000) |
| Trading | ✅ Places orders | ❌ Read-only |
| Best For | Production server | Local monitoring |

## 💡 Use Cases

### Use Case 1: Server + Local Monitoring
```bash
# On server: Run watch for trading
./mexc-grid-trader watch --config config.json

# On local: Run report for dashboard
./mexc-grid-trader report --config config.json
```
Result: Server collects trading data, local collects monitoring data independently.

### Use Case 2: Local Development
```bash
# Just run report for testing
./mexc-grid-trader report --config config.json
```
Result: Collect data locally without affecting server.

### Use Case 3: Multiple Environments
```bash
# Dev environment
./mexc-grid-trader report --config config-dev.json --port 3000

# Staging environment
./mexc-grid-trader report --config config-staging.json --port 3001
```
Result: Each environment tracks its own performance independently.

## ⚠️ Troubleshooting

### Problem: No PnL summary after waiting
**Cause:** Need at least 2 snapshots with different balances
**Solution:** Wait longer or check if trades are happening

### Problem: Average price card not showing
**Cause:** `baseChange` is 0 (no token balance change)
**Solution:** Wait for trades to execute and change balances

### Problem: Old data showing
**Cause:** Browser cache
**Solution:** Hard refresh (Ctrl+Shift+R or Cmd+Shift+R)

### Problem: Snapshots not being saved
**Cause:** File permission issues or disk full
**Solution:** Check logs and file permissions

## 🔮 Future Enhancements

Possible improvements:
- [ ] Configurable snapshot interval
- [ ] Snapshot cleanup/rotation (auto-delete old data)
- [ ] Export snapshots to CSV
- [ ] Import snapshots from server
- [ ] Snapshot compression
- [ ] Multi-symbol support in one snapshot file

## 📚 Related Documentation

- [REPORT_DASHBOARD.md](REPORT_DASHBOARD.md) - Full dashboard guide
- [AVERAGE_PRICE_FEATURE.md](AVERAGE_PRICE_FEATURE.md) - Average price details
- [README.md](README.md) - General project documentation

## 🎉 Summary

The report command now gives you **complete independence**:
- ✅ No need to rely on server snapshots
- ✅ No need to copy files between machines
- ✅ Real-time local monitoring with full PnL tracking
- ✅ Automatic average price analysis
- ✅ Clean separation between production and monitoring data

Just run `report` and everything works out of the box! 🚀
