# 充值/提现调整功能

## 🎯 问题

当您在交易期间进行充值或提现操作时，系统会把这些资金变动误认为是交易盈亏，导致 PnL 计算不准确。

### 示例问题场景

```
开始余额：1000 USDT
充值：    +500 USDT
结束余额：1600 USDT

❌ 错误计算：盈利 = 1600 - 1000 = 600 USDT
✅ 正确计算：盈利 = 1600 - 1000 - 500 = 100 USDT
```

## ✨ 解决方案

使用 `adjust` 命令记录充值/提现，系统会自动在计算 PnL 时排除这些外部资金变动。

## 📝 使用方法

### 1. 记录充值

```bash
# 充值 500 USDT 到 MEXC
./target/release/mexc-grid-trader adjust \
  --exchange mexc \
  --asset USDT \
  --amount 500 \
  --note "Initial funding"

# 充值代币
./target/release/mexc-grid-trader adjust \
  --exchange gate \
  --asset ZKWASM \
  --amount 10000 \
  --note "Added more ZKWASM"
```

### 2. 记录提现

```bash
# 提现 200 USDT（使用负数）
./target/release/mexc-grid-trader adjust \
  --exchange mexc \
  --asset USDT \
  --amount -200 \
  --note "Withdrew profits"
```

### 3. 查看所有调整记录

```bash
# 查看所有交易所的调整记录
./target/release/mexc-grid-trader adjustments

# 查看特定交易所
./target/release/mexc-grid-trader adjustments --exchange mexc
```

## 🔄 工作原理

### Step 1: 记录调整

当您使用 `adjust` 命令时，系统会：
1. 保存充值/提现记录到文件 `.adjustments_<exchange>_<symbol>.jsonl`
2. 记录时间戳、资产、金额和备注

### Step 2: 计算 PnL 时自动调整

```rust
// 原始资产变化
raw_change = later_balance - earlier_balance

// 获取期间的净调整（充值 - 提现）
net_adjustment = deposits - withdrawals

// 实际盈亏 = 原始变化 - 净调整
actual_pnl = raw_change - net_adjustment
```

### 示例计算

```
时间段：10:00 - 12:00

资产变化：
  USDT: 1000 → 1600 (+600)

调整记录：
  11:00 - 充值 +500 USDT

PnL 计算：
  原始变化 = +600 USDT
  净调整   = +500 USDT
  实际盈亏 = +600 - 500 = +100 USDT  ✅
```

## 💡 最佳实践

### 1. 及时记录

**立即记录充值/提现，不要等待！**

```bash
# ❌ 不好 - 几天后才记录
# 可能忘记具体金额和时间

# ✅ 好 - 充值后立即记录
./target/release/mexc-grid-trader adjust -e mexc -a USDT -m 500 -n "2024-01-15 充值"
```

### 2. 添加详细备注

```bash
# ✅ 好的备注示例
--note "Initial capital investment"
--note "Profit withdrawal for month-end"
--note "Added funds for grid expansion"
--note "Transfer from CEX to prepare for trade"

# ❌ 不好的备注
--note "money"
--note "test"
```

### 3. 定期检查

```bash
# 每周检查一次调整记录
./target/release/mexc-grid-trader adjustments

# 确保所有充值/提现都已记录
```

### 4. 交易前后对比

```bash
# 充值前查看余额
./target/release/mexc-grid-trader orders

# 充值
# ...

# 充值后立即记录
./target/release/mexc-grid-trader adjust -e mexc -a USDT -m 1000

# 再次查看余额确认
./target/release/mexc-grid-trader orders
```

## 📊 报告中的显示

使用调整功能后，PnL 报告会显示：

```
╔════════════════════════════════════════════════════════╗
║              PROFIT & LOSS SUMMARY                     ║
╚════════════════════════════════════════════════════════╝

📅 Time Period:
  From: 2024-01-15 10:00:00 UTC
  To:   2024-01-15 12:00:00 UTC
  Duration: 7200 seconds (120.0 minutes)

💸 Deposits/Withdrawals (excluded from P&L):
  📥 Deposit USDT: +500.00
  📤 Withdrawal ZKWASM: -1000.00000000

💰 Total Value Change (in USDT):
  Before: 1000.00
  After:  1600.00
  📈 Net P&L: +100.00 (+10.000%)
  ℹ️  (Adjusted for deposits/withdrawals)

🪙 Asset Changes:
  ⬆ ZKWASM   10000.00000000 → 11000.00000000 (+1000.00000000)
  ⬆ USDT        1000.00 →    1100.00 (+100.00)
```

## 🗂️ 文件结构

调整记录保存在 JSONL 格式文件中：

```
.adjustments_mexc_ZKWASMUSDT.jsonl
.adjustments_gate_ZKWASMUSDT.jsonl
.adjustments_kucoin_ZKWASMUSDT.jsonl
```

每条记录：

```json
{
  "timestamp": 1705315200,
  "datetime": "2024-01-15 10:00:00 UTC",
  "exchange": "mexc",
  "asset": "USDT",
  "amount": 500.0,
  "adjustment_type": "Deposit",
  "note": "Initial funding"
}
```

## ⚙️ 命令参考

### adjust 命令

```bash
./target/release/mexc-grid-trader adjust [OPTIONS]

Options:
  -e, --exchange <EXCHANGE>  交易所名称（如果不指定，会提示选择）
  -a, --asset <ASSET>        资产名称（如 USDT, BTC 等）[必需]
  -m, --amount <AMOUNT>      金额（正数=充值，负数=提现）[必需]
  -n, --note <NOTE>          备注信息
  --config <CONFIG>          配置文件路径 [默认: config.json]
```

### adjustments 命令

```bash
./target/release/mexc-grid-trader adjustments [OPTIONS]

Options:
  -e, --exchange <EXCHANGE>  只显示指定交易所的记录
  --config <CONFIG>          配置文件路径 [默认: config.json]
```

## ❓ 常见问题

### Q: 忘记记录充值怎么办？
**A:** 可以随时补充记录，时间戳会记录为当前时间。虽然不如即时记录准确，但总比不记录好。

### Q: 记录错误了怎么办？
**A:** 手动编辑 `.adjustments_<exchange>_<symbol>.jsonl` 文件，删除或修改错误的行。

### Q: 如何删除所有调整记录？
**A:** 删除 `.adjustments_*.jsonl` 文件即可。

### Q: 调整记录会影响实际交易吗？
**A:** 不会！调整记录仅用于 PnL 计算，不影响实际交易。

### Q: 是否需要为每个交易所分别记录？
**A:** 是的，每个交易所的调整记录是独立的。

## 🎯 总结

使用调整功能可以：
- ✅ 准确计算交易盈亏
- ✅ 排除充值/提现的影响
- ✅ 获得真实的策略表现数据
- ✅ 更好地评估交易效果

记住：**每次充值或提现后立即记录！**
