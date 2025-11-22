# 调整记录验证功能

## 🎯 功能概述

当启动 `report` 服务时，系统会自动检查和验证所有的充值/提现调整记录，帮助您确保 PnL 计算的准确性。

## ✨ 自动验证

### 启动时检查

```bash
./target/release/mexc-grid-trader report
```

**输出示例：**

```
🔍 Verifying adjustment records...

╔════════════════════════════════════════════════════════╗
║       ADJUSTMENT VERIFICATION & RECONCILIATION         ║
╚════════════════════════════════════════════════════════╝

📊 mexc - Found 3 adjustment(s)
────────────────────────────────────────────────────────

  Summary by Asset:
    USDT ────────────────────────────────────────────────
      📥 Total Deposits:         500.00000000
      📤 Total Withdrawals:      200.00000000
      ➕ Net Change:             300.00000000
    ZKWASM ──────────────────────────────────────────────
      📥 Total Deposits:       10000.00000000
      ➕ Net Change:           10000.00000000

  🔍 Verifying with exchange API...
  ✓ USDT: Initial=1000.0000, Current=1350.0000, Adjustments=+300.0000, Trading P&L=+50.0000
  ✓ ZKWASM: Initial=5000.0000, Current=15200.0000, Adjustments=+10000.0000, Trading P&L=+200.0000

📊 gate - Found 1 adjustment(s)
────────────────────────────────────────────────────────

  Summary by Asset:
    USDT ────────────────────────────────────────────────
      📥 Total Deposits:        1000.00000000
      ➕ Net Change:            1000.00000000

  🔍 Verifying with exchange API...
  ✓ USDT: Initial=500.0000, Current=1520.0000, Adjustments=+1000.0000, Trading P&L=+20.0000

════════════════════════════════════════════════════════
✅ Verification complete: 4 total adjustment(s) found
💚 All adjustments appear consistent
════════════════════════════════════════════════════════

🚀 API Server starting on http://0.0.0.0:3000
📊 Report endpoint: http://localhost:3000/api/report
💚 Health check: http://localhost:3000/health
⚠️  Press Ctrl+C to stop
```

## 📊 验证内容

### 1. 调整记录汇总

系统会显示每个交易所的：
- 总充值金额
- 总提现金额
- 净变化（充值 - 提现）

### 2. 按资产分类

每个资产（如 USDT, ZKWASM）都会单独统计

### 3. API 验证（可选）

如果存在初始快照，系统会：
1. 获取当前账户余额
2. 计算实际余额变化
3. 对比调整记录
4. 计算交易盈亏

**公式：**
```
交易盈亏 = (当前余额 - 初始余额) - 调整净额
```

## 💡 验证逻辑

### 示例场景

```
初始快照（10:00）:
  USDT: 1000.00

调整记录:
  11:00 - 充值 +500 USDT
  13:00 - 提现 -200 USDT
  净调整: +300 USDT

当前余额（15:00）:
  USDT: 1350.00

验证计算:
  余额变化 = 1350 - 1000 = +350 USDT
  调整净额 = +300 USDT
  交易盈亏 = +350 - 300 = +50 USDT  ✅
```

### 验证结果解读

```
✓ USDT: Initial=1000.0000, Current=1350.0000, Adjustments=+300.0000, Trading P&L=+50.0000
```

- **Initial**: 初始余额（来自第一个快照）
- **Current**: 当前余额（通过API获取）
- **Adjustments**: 调整总和（充值-提现）
- **Trading P&L**: 实际交易盈亏

## ⚠️ 注意事项

### 1. 无快照时

如果没有历史快照，会显示：
```
✓ No baseline snapshot found for comparison
```

这是正常的，表示无法对账（因为没有初始基准）。

### 2. API 访问失败

如果无法访问交易所API或没有权限查看历史，会显示：
```
⚠️  Could not verify via API: Permission denied
ℹ️  This is normal if you don't have access to deposit/withdrawal history
```

这不影响系统继续运行。

### 3. 发现差异

如果发现余额不匹配，系统会：
- 显示 ⚠️ 警告
- 提示检查调整记录
- 但继续启动服务

## 🔍 手动验证

如果自动验证发现问题，您可以：

### 1. 检查调整记录文件

```bash
cat .adjustments_mexc_ZKWASMUSDT.jsonl
```

### 2. 使用命令查看

```bash
./target/release/mexc-grid-trader adjustments --exchange mexc
```

### 3. 手动对账

1. 登录交易所查看充值/提现历史
2. 对比 `.adjustments_*.jsonl` 文件
3. 补充遗漏的记录：
   ```bash
   ./target/release/mexc-grid-trader adjust -e mexc -a USDT -m 500 -n "Missed deposit"
   ```

## 🛠️ 故障排除

### 问题：验证失败

**可能原因：**
1. 遗漏了某次充值/提现记录
2. 记录金额错误
3. 时间戳不准确

**解决方法：**
```bash
# 1. 查看所有调整记录
./target/release/mexc-grid-trader adjustments

# 2. 检查交易所充值/提现历史

# 3. 补充或修正记录
# 补充遗漏的充值
./target/release/mexc-grid-trader adjust -e mexc -a USDT -m 500

# 修正错误（手动编辑文件）
nano .adjustments_mexc_ZKWASMUSDT.jsonl
```

### 问题：Trading P&L 为负数但实际盈利

**可能原因：**
充值记录遗漏，系统把充值金额当成了交易亏损。

**解决方法：**
```bash
# 补充遗漏的充值记录
./target/release/mexc-grid-trader adjust -e mexc -a USDT -m 1000 -n "Initial deposit (missed)"
```

### 问题：Trading P&L 为正数但实际亏损

**可能原因：**
提现记录遗漏，系统把提现金额当成了交易盈利。

**解决方法：**
```bash
# 补充遗漏的提现记录
./target/release/mexc-grid-trader adjust -e mexc -a USDT -m -500 -n "Withdrawal (missed)"
```

## 📈 最佳实践

### 1. 定期对账

建议每周运行一次 report 命令，检查验证结果：
```bash
./target/release/mexc-grid-trader report
# 查看验证输出
# Ctrl+C 退出
```

### 2. 及时记录

每次充值/提现后立即记录：
```bash
# 充值后
./target/release/mexc-grid-trader adjust -e mexc -a USDT -m 500 -n "$(date)"

# 提现后
./target/release/mexc-grid-trader adjust -e mexc -a USDT -m -200 -n "$(date)"
```

### 3. 备份记录

定期备份调整记录文件：
```bash
cp .adjustments_*.jsonl ~/backups/
```

### 4. 交叉验证

对比多个数据源：
- 调整记录文件
- 交易所充值/提现历史
- 银行/钱包转账记录

## 🎯 总结

调整记录验证功能：
- ✅ 自动在 report 启动时运行
- ✅ 汇总所有充值/提现
- ✅ 通过 API 验证余额变化
- ✅ 计算实际交易盈亏
- ✅ 帮助发现记录遗漏或错误

**确保准确的 PnL 计算，从验证调整记录开始！**
