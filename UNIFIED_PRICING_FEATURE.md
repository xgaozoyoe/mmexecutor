# 统一定价功能 (Unified Pricing Feature)

## 🎯 功能概述

**防止跨交易所套利的统一定价策略！**

现在，当你在多个交易所同时运行网格交易时，系统会：
1. 先计算每个交易所各自的目标订单价格
2. 对所有交易所的**同一档位**订单价格取平均
3. 使用统一的平均价格在所有交易所下单

这样可以防止别人在你的交易所之间进行套利！

## ❓ 为什么需要这个功能？

### 问题场景

**之前的做法（独立定价）：**
```
MEXC:    买单 @ 0.015000  |  卖单 @ 0.015300
Gate.io: 买单 @ 0.015100  |  卖单 @ 0.015400
KuCoin:  买单 @ 0.014900  |  卖单 @ 0.015200
```

❌ **问题**：套利者可以：
1. 在 KuCoin 以 0.015200 从你这里买入
2. 在 Gate.io 以 0.015400 卖给你
3. 无风险赚取差价 0.015400 - 0.015200 = 0.000200

### 解决方案

**现在的做法（统一定价）：**
```
计算平均价格：
  买单平均 = (0.015000 + 0.015100 + 0.014900) / 3 = 0.015000
  卖单平均 = (0.015300 + 0.015400 + 0.015200) / 3 = 0.015300

所有交易所使用统一价格：
MEXC:    买单 @ 0.015000  |  卖单 @ 0.015300
Gate.io: 买单 @ 0.015000  |  卖单 @ 0.015300
KuCoin:  买单 @ 0.015000  |  卖单 @ 0.015300
```

✅ **好处**：
- 消除跨交易所价差
- 防止套利者从你这里无风险获利
- 所有交易所订单价格一致

## 🔄 工作流程

### Step 1: 收集各交易所的目标订单价格
```
📊 Step 1: Calculating target order prices for each exchange...
  ✅ MEXC: 10 buy orders, 10 sell orders
  ✅ Gate.io: 10 buy orders, 10 sell orders
  ✅ KuCoin: 10 buy orders, 10 sell orders
```

系统会：
1. 连接到每个交易所
2. 获取市场数据（订单簿）
3. 根据配置的策略计算该交易所的目标订单价格
4. 考虑网格间距、偏移量等参数

### Step 2: 计算统一的平均价格
```
💰 Step 2: Calculating unified order prices (average across all exchanges)
  📝 Unified buy orders: 10
  📝 Unified sell orders: 10
  🔒 Using averaged prices to prevent cross-exchange arbitrage
```

对每一档订单：
```
第1档买单:
  MEXC:    0.015000
  Gate.io: 0.015100  →  平均: 0.015000
  KuCoin:  0.014900

第1档卖单:
  MEXC:    0.015300
  Gate.io: 0.015400  →  平均: 0.015300
  KuCoin:  0.015200
```

### Step 3: 在所有交易所使用统一价格下单
```
📝 Step 3: Placing unified orders on all exchanges...

╔═══════════════════════════════════════════════════════════╗
║  Processing Exchange 1/3: MEXC                            ║
╚═══════════════════════════════════════════════════════════╝

📋 Using unified orders (averaged across all exchanges):
  Buy orders: 10
  Sell orders: 10
  First buy order: 0.01500000 USDT
  First sell order: 0.01530000 USDT

📝 Placing 20 unified orders...
  [1/20] BUY ZKWASMUSDT @ 0.01500000 (qty: 100.00) ... ✅ Order ID: 12345
  [2/20] BUY ZKWASMUSDT @ 0.01495000 (qty: 120.00) ... ✅ Order ID: 12346
  ...
```

每个交易所都使用**相同的**统一价格！

## 📊 实际示例

### 配置文件
```json
{
  "exchanges": [
    {
      "name": "mexc",
      "api_key": "...",
      "api_secret": "..."
    },
    {
      "name": "gate",
      "api_key": "...",
      "api_secret": "..."
    },
    {
      "name": "kucoin",
      "api_key": "...",
      "api_secret": "...",
      "api_passphrase": "..."
    }
  ],
  "grid": {
    "symbol": "ZKWASMUSDT",
    "grid_levels": 10,
    "grid_interval_percentage": 0.3,
    ...
  }
}
```

### 运行命令
```bash
./target/release/mexc-grid-trader watch --config config.json
```

### 输出示例
```
═══════════════════════════════════════════════════════════════
  Configured exchanges: 3
    1. mexc
    2. gate
    3. kucoin
═══════════════════════════════════════════════════════════════

📊 Step 1: Calculating target order prices for each exchange...
  ✅ mexc: 10 buy orders, 10 sell orders
  ✅ gate: 10 buy orders, 10 sell orders
  ✅ kucoin: 10 buy orders, 10 sell orders

💰 Step 2: Calculating unified order prices (average across all exchanges)
  📝 Unified buy orders: 10
  📝 Unified sell orders: 10
  🔒 Using averaged prices to prevent cross-exchange arbitrage

📝 Step 3: Placing unified orders on all exchanges...

╔═══════════════════════════════════════════════════════════════╗
║  Processing Exchange 1/3: mexc                               ║
╚═══════════════════════════════════════════════════════════════╝

Exchange: Mexc
💼 Account Information:
  ZKWASM balance: 100000.0000 (free: 95000.0, locked: 5000.0)
  USDT balance: 5000.00 (free: 4500.00, locked: 500.00)

📋 Using unified orders (averaged across all exchanges):
  Buy orders: 10
  Sell orders: 10
  First buy order: 0.01500000 USDT
  First sell order: 0.01530000 USDT

📊 Current open orders: 15
🔄 Canceling 15 existing orders...
  ✅ Canceled existing orders

📝 Placing 20 unified orders...
  [1/20] BUY ZKWASMUSDT @ 0.01500000 (qty: 100.00000) ... ✅ Order ID: 12345
  [2/20] BUY ZKWASMUSDT @ 0.01495000 (qty: 120.00000) ... ✅ Order ID: 12346
  ...
  [20/20] SELL ZKWASMUSDT @ 0.01575000 (qty: 85.00000) ... ✅ Order ID: 12364

📊 Order placement summary:
  ✅ Successful: 20
  ❌ Failed: 0

✅ Successfully processed mexc

[继续处理 Gate.io 和 KuCoin，使用相同的统一价格...]
```

## 🎯 关键优势

### 1. 防止套利
❌ **之前**：套利者可以在你的交易所之间赚取无风险利润
✅ **现在**：所有交易所价格一致，无套利空间

### 2. 公平定价
所有交易所的订单使用相同的价格基准，更加公平合理

### 3. 简化管理
不需要为每个交易所单独调整定价策略

### 4. 自动计算
系统自动计算平均价格，无需手动干预

## ⚙️ 技术实现

### 核心函数

#### 1. `calculate_exchange_target_orders`
为单个交易所计算目标订单价格：
```rust
async fn calculate_exchange_target_orders(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
) -> Result<Vec<order_calculator::GridOrder>>
```

功能：
- 获取交易所的订单簿数据
- 根据策略计算中间价
- 生成网格订单（买单 + 卖单）

#### 2. `calculate_unified_orders`
计算统一的平均订单价格：
```rust
fn calculate_unified_orders(
    exchange_orders_list: &[(String, Vec<order_calculator::GridOrder>)],
) -> Result<Vec<order_calculator::GridOrder>>
```

功能：
- 对所有交易所的买单按档位分组
- 对所有交易所的卖单按档位分组
- 计算每一档的平均价格和数量

#### 3. `place_unified_orders_for_exchange`
使用统一价格在交易所下单：
```rust
async fn place_unified_orders_for_exchange(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
    unified_orders: &[order_calculator::GridOrder],
    auto_mode: bool,
    iteration: Option<u64>,
) -> Result<()>
```

功能：
- 取消现有订单
- 使用统一价格下新订单
- 保存快照数据

### 数据流

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│  MEXC    │     │ Gate.io  │     │  KuCoin  │
│ 订单簿   │     │ 订单簿   │     │ 订单簿   │
└────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │
     ▼                ▼                ▼
┌──────────┐     ┌──────────┐     ┌──────────┐
│ 中间价   │     │ 中间价   │     │ 中间价   │
│ 计算     │     │ 计算     │     │ 计算     │
└────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │
     ▼                ▼                ▼
┌──────────┐     ┌──────────┐     ┌──────────┐
│ 网格     │     │ 网格     │     │ 网格     │
│ 订单列表 │     │ 订单列表 │     │ 订单列表 │
└────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │
     └────────┬───────┴────────┬───────┘
              ▼                ▼
         ┌─────────────────────────┐
         │   计算统一平均价格       │
         │  (按档位对齐并平均)      │
         └───────────┬─────────────┘
                     │
                     ▼
         ┌─────────────────────────┐
         │   统一订单列表           │
         │  (所有交易所使用)        │
         └───────────┬─────────────┘
                     │
       ┌─────────────┼─────────────┐
       │             │             │
       ▼             ▼             ▼
  ┌────────┐   ┌────────┐   ┌────────┐
  │  MEXC  │   │Gate.io │   │ KuCoin │
  │ 下单   │   │ 下单   │   │ 下单   │
  └────────┘   └────────┘   └────────┘
```

## 📝 使用说明

### 1. 确保配置多个交易所
```json
{
  "exchanges": [
    // 至少配置 2 个交易所才能体现统一定价的优势
    { "name": "mexc", ... },
    { "name": "gate", ... },
    { "name": "kucoin", ... }
  ]
}
```

### 2. 运行 watch 命令
```bash
./target/release/mexc-grid-trader watch --config config.json --interval 120
```

### 3. 观察日志
看到 3 个步骤：
1. 计算各交易所目标价格
2. 计算统一平均价格
3. 使用统一价格下单

## ⚠️ 注意事项

### 1. 订单档位对齐
- 系统会使用最小的订单数量（如果交易所订单数不同）
- 例如：MEXC 有 10 档，Gate.io 有 12 档 → 使用 10 档统一价格

### 2. 单交易所场景
- 如果只配置 1 个交易所，系统会自动使用该交易所的价格
- 不会进行平均计算（因为只有一个数据源）

### 3. 价格精度
- 统一价格保持较高精度（8 位小数）
- 满足各交易所的下单要求

### 4. 失败处理
- 如果某个交易所获取价格失败，会跳过该交易所
- 使用其他成功的交易所数据继续计算平均值

## 🔮 未来优化

可能的改进：
- [ ] 加权平均（根据交易所流动性权重）
- [ ] 异常值过滤（排除偏离过大的价格）
- [ ] 自定义平均算法（中位数、加权中位数等）
- [ ] 实时价差监控和告警
- [ ] 统一价格的历史记录和分析

## 📚 相关文档

- [README.md](README.md) - 项目总览
- [REPORT_DASHBOARD.md](REPORT_DASHBOARD.md) - 监控面板
- [AVERAGE_PRICE_FEATURE.md](AVERAGE_PRICE_FEATURE.md) - 平均交易价格

## 🎉 总结

统一定价功能通过对所有交易所的订单价格取平均，有效防止了跨交易所套利，保护你的交易策略不被他人利用。

**核心理念**：
> 不让别人在你的交易所之间低买高卖赚取无风险利润！

使用统一定价，让你的多交易所网格交易更加安全和高效！🚀
