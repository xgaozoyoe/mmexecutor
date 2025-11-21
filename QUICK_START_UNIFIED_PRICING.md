# 快速开始 - 统一定价功能

## 🎯 一句话说明

**所有交易所使用相同的订单价格，防止套利者在你的交易所之间低买高卖！**

## 🚀 如何使用

### 1. 配置多个交易所

确保你的 `config.json` 中配置了多个交易所：

```json
{
  "exchanges": [
    {
      "name": "mexc",
      "api_key": "your_mexc_key",
      "api_secret": "your_mexc_secret"
    },
    {
      "name": "gate",
      "api_key": "your_gate_key",
      "api_secret": "your_gate_secret"
    },
    {
      "name": "kucoin",
      "api_key": "your_kucoin_key",
      "api_secret": "your_kucoin_secret",
      "api_passphrase": "your_kucoin_passphrase"
    }
  ],
  "grid": {
    "symbol": "ZKWASMUSDT",
    ...
  }
}
```

### 2. 运行 watch 命令

```bash
./target/release/mexc-grid-trader watch --config config.json
```

### 3. 观察输出

你会看到 4 个步骤：

```
📊 Step 1: Collecting mid prices from all exchanges...
  ✅ mexc: 0.015150
  ✅ gate: 0.015250
  ✅ kucoin: 0.015050

💰 Step 2: Calculating unified mid price
  Unified mid price (average): 0.015150
  🔒 Using averaged mid price to prevent cross-exchange arbitrage

📝 Step 3: Calculating grid orders using unified mid price...
  Using mid price: 0.015150
  Total orders: 20 (10 buy + 10 sell)

📝 Step 4: Placing unified orders on all exchanges...
```

就这么简单！🎉

## 📊 实际效果

### 之前（独立定价）

```
MEXC:    买 @ 0.015000  卖 @ 0.015300
Gate.io: 买 @ 0.015100  卖 @ 0.015400  ← 套利者可以赚 0.0002！
KuCoin:  买 @ 0.014900  卖 @ 0.015200
```

❌ **问题**：套利者在 KuCoin 买，在 Gate.io 卖，赚取差价

### 现在（统一定价）

```
MEXC:    买 @ 0.015000  卖 @ 0.015300
Gate.io: 买 @ 0.015000  卖 @ 0.015300  ← 所有价格一致！
KuCoin:  买 @ 0.015000  卖 @ 0.015300
```

✅ **解决**：没有价差，套利者无法获利

## 💡 工作原理

### 简单说明

1. **收集中间价**：从每个交易所获取当前的市场中间价
2. **计算平均中间价**：把所有交易所的中间价取平均
3. **生成网格订单**：使用平均中间价生成网格订单（只计算一次）
4. **统一下单**：所有交易所使用完全相同的网格订单

### 举例

```
Step 1 - 收集中间价：
  MEXC 中间价:    0.015150
  Gate 中间价:    0.015250
  KuCoin 中间价:  0.015050

Step 2 - 计算平均：
  统一中间价 = (0.015150 + 0.015250 + 0.015050) / 3 = 0.015150

Step 3 - 生成网格订单（只计算一次）：
  根据统一中间价 0.015150 和网格策略，生成：
  - 10 个买单
  - 10 个卖单

Step 4 - 所有交易所使用相同订单：
  MEXC、Gate、KuCoin 都下相同的 20 个订单
```

## ✅ 优势

1. **防套利**：消除交易所间价差
2. **自动化**：无需手动调整
3. **公平性**：基于所有交易所的市场数据
4. **简单**：配置多个交易所就自动启用

## ⚙️ 无需额外配置

- ✅ 自动检测多个交易所
- ✅ 自动计算平均价格
- ✅ 自动应用统一定价
- ✅ 单交易所时保持原有行为

## 📝 查看日志

运行时注意这些关键信息：

```bash
# Step 1 - 显示每个交易所的中间价
✅ mexc: 0.015150
✅ gate: 0.015250

# Step 2 - 显示统一中间价
💰 Unified mid price (average): 0.015150
🔒 Using averaged mid price to prevent cross-exchange arbitrage

# Step 3 - 显示生成的网格订单
📝 Using mid price: 0.015150
📝 Total orders: 20 (10 buy + 10 sell)

# Step 4 - 每个交易所都使用统一订单
📋 Using unified grid orders:
  First buy order: 0.01500000 USDT
  First sell order: 0.01530000 USDT
```

## 🔍 验证是否生效

### 方法 1：查看日志

看到 "Using averaged mid price to prevent cross-exchange arbitrage" 就说明启用了

### 方法 2：检查订单

登录各个交易所查看实际下单价格，应该完全一致（每一档订单的价格都相同）

### 方法 3：对比价格

```bash
# 查看日志中的订单价格
grep "BUY ZKWASMUSDT" logs.txt

# 所有交易所的相同档位订单价格应该完全一样！
```

## ❓ 常见问题

### Q: 只有1个交易所会怎样？
**A:** 自动使用该交易所的中间价，不进行平均（因为只有一个数据源）

### Q: 某个交易所中间价获取失败怎么办？
**A:** 使用其他成功的交易所计算平均中间价，继续执行

### Q: 会影响原有策略吗？
**A:** 不会！网格间距、偏移量等配置完全保留，只是基于平均中间价生成订单

### Q: 需要重新配置吗？
**A:** 不需要！只要配置了多个交易所就自动启用

### Q: 和之前的订单平均有什么不同？
**A:** 更简单！之前是先生成各交易所的订单再平均（复杂），现在是先平均中间价再生成订单（简单）

## 🎯 最佳实践

### 1. 使用相似的交易所
- ✅ 都是现货交易
- ✅ 流动性相近
- ✅ 同一时区（减少延迟）

### 2. 监控价差
定期检查各交易所的实际市场价格，确保差异不大

### 3. 合理数量
建议配置 2-4 个交易所：
- 太少：统一定价意义不大
- 太多：可能某些交易所数据异常影响平均值

## 📚 更多信息

- **详细文档**: [UNIFIED_PRICING_FEATURE.md](UNIFIED_PRICING_FEATURE.md)
- **技术实现**: 查看代码中的 `calculate_unified_orders()`
- **项目文档**: [README.md](README.md)

## 🎉 总结

统一定价功能让你可以：
- ✅ 安全地在多个交易所运行网格
- ✅ 不用担心被套利
- ✅ 无需额外配置
- ✅ 自动化运行

只需配置多个交易所，然后正常运行 watch 命令即可！🚀
