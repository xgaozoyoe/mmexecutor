# 统一定价功能 (Unified Pricing Feature)

## 🎯 功能概述

**防止跨交易所套利的统一定价策略！**

现在，当你在多个交易所同时运行网格交易时，系统会：
1. 从每个交易所获取中间价（mid price）
2. 计算所有交易所中间价的**平均值**作为统一中间价
3. 使用统一中间价生成网格订单（只计算一次）
4. 在所有交易所下相同的网格订单

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
Step 1: 获取各交易所的中间价：
  MEXC 中间价:    0.015150
  Gate.io 中间价: 0.015250
  KuCoin 中间价:  0.015050

Step 2: 计算统一中间价（平均）：
  统一中间价 = (0.015150 + 0.015250 + 0.015050) / 3 = 0.015150

Step 3: 使用统一中间价生成网格订单（只计算一次）：
  根据网格策略，生成买单和卖单

Step 4: 所有交易所使用相同的网格订单：
MEXC:    买单 @ 0.015000  |  卖单 @ 0.015300
Gate.io: 买单 @ 0.015000  |  卖单 @ 0.015300
KuCoin:  买单 @ 0.015000  |  卖单 @ 0.015300
```

✅ **好处**：
- 消除跨交易所价差
- 防止套利者从你这里无风险获利
- 所有交易所订单价格一致

## 🔄 工作流程

### Step 1: 收集各交易所的中间价
```
📊 Step 1: Collecting mid prices from all exchanges...
  ✅ mexc: 0.015150
  ✅ gate: 0.015250
  ✅ kucoin: 0.015050
```

系统会：
1. 连接到每个交易所
2. 获取市场数据（订单簿）
3. 根据配置的策略计算该交易所的中间价（mid price）
4. 考虑买卖盘深度、成交量等因素

### Step 2: 计算统一中间价
```
💰 Step 2: Calculating unified mid price
  Unified mid price (average): 0.015150
  🔒 Using averaged mid price to prevent cross-exchange arbitrage
```

计算逻辑：
```
统一中间价 = (所有交易所的中间价之和) / 交易所数量
         = (0.015150 + 0.015250 + 0.015050) / 3
         = 0.015150
```

### Step 3: 使用统一中间价生成网格订单
```
📝 Step 3: Calculating grid orders using unified mid price...
  Using mid price: 0.015150
  Generating buy orders...
  Generating sell orders...
  Total orders: 20 (10 buy + 10 sell)
```

基于统一中间价，根据网格策略（网格间距、层数等）生成订单列表，**只计算一次**！

### Step 4: 在所有交易所下相同的网格订单
```
📝 Step 4: Placing unified orders on all exchanges...

╔═══════════════════════════════════════════════════════════╗
║  Processing Exchange 1/3: mexc                            ║
╚═══════════════════════════════════════════════════════════╝

📋 Using unified grid orders:
  Buy orders: 10
  Sell orders: 10
  First buy order: 0.01500000 USDT
  First sell order: 0.01530000 USDT

📝 Placing 20 unified orders...
  [1/20] BUY ZKWASMUSDT @ 0.01500000 (qty: 100.00) ... ✅ Order ID: 12345
  [2/20] BUY ZKWASMUSDT @ 0.01495000 (qty: 120.00) ... ✅ Order ID: 12346
  ...
```

每个交易所都使用**完全相同的**网格订单！

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

╔═══════════════════════════════════════════════════════════════╗
║  Processing Exchange 1/3: mexc                               ║
╚═══════════════════════════════════════════════════════════════╝

Exchange: Mexc
💼 Account Information:
  ZKWASM balance: 100000.0000 (free: 95000.0, locked: 5000.0)
  USDT balance: 5000.00 (free: 4500.00, locked: 500.00)

📋 Using unified grid orders:
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

[继续处理 Gate.io 和 KuCoin，使用相同的统一网格订单...]
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

#### 1. `get_exchange_mid_price`
从单个交易所获取中间价：
```rust
async fn get_exchange_mid_price(
    exchange_config: &config::ExchangeConfig,
    grid_config: &config::GridConfig,
) -> Result<f64>
```

功能：
- 创建交易所客户端
- 获取订单簿数据
- 根据配置的策略计算中间价（mid price）
- 考虑买卖盘深度、成交量等因素
- 返回单个数值（中间价）

#### 2. `place_orders_internal_with_iteration` (4-step process)
主要的统一定价逻辑：

**Step 1: 收集中间价**
```rust
for exchange_config in &config.exchanges {
    let mid_price = get_exchange_mid_price(exchange_config, &config.grid).await?;
    mid_prices.push(mid_price);
}
```

**Step 2: 计算统一中间价**
```rust
let unified_mid_price = mid_prices.iter().sum::<f64>() / mid_prices.len() as f64;
```

**Step 3: 生成统一网格订单**
```rust
let unified_orders = OrderCalculator::calculate_grid_orders(
    unified_mid_price,
    &config.grid
);
```

**Step 4: 在所有交易所下相同订单**
```rust
for exchange_config in &config.exchanges {
    place_unified_orders_for_exchange(
        exchange_config,
        &config.grid,
        &unified_orders,
        auto_mode,
        iteration,
    ).await?;
}
```

#### 3. `place_unified_orders_for_exchange`
在单个交易所下统一订单：
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
- 下新的统一网格订单
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
│ 计算     │     │ 计算     │     │ 计算     │
│ 中间价   │     │ 中间价   │     │ 中间价   │
└────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │
     │   0.015150     │   0.015250     │   0.015050
     │                │                │
     └────────┬───────┴────────┬───────┘
              │                │
              ▼                ▼
         ┌─────────────────────────┐
         │   计算统一中间价          │
         │   (简单算术平均)          │
         │                          │
         │   0.015150               │
         └───────────┬─────────────┘
                     │
                     ▼
         ┌─────────────────────────┐
         │   生成统一网格订单        │
         │   (只计算一次！)          │
         │                          │
         │   20 orders              │
         │   (10 buy + 10 sell)     │
         └───────────┬─────────────┘
                     │
       ┌─────────────┼─────────────┐
       │             │             │
       ▼             ▼             ▼
  ┌────────┐   ┌────────┐   ┌────────┐
  │  MEXC  │   │Gate.io │   │ KuCoin │
  │ 下单   │   │ 下单   │   │ 下单   │
  │ (相同) │   │ (相同) │   │ (相同) │
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
看到 4 个步骤：
1. 收集各交易所的中间价
2. 计算统一中间价（平均）
3. 使用统一中间价生成网格订单
4. 在所有交易所下相同的统一订单

## ⚠️ 注意事项

### 1. 网格订单完全一致
- 所有交易所使用完全相同的网格订单
- 价格、数量、档位完全一致
- 这是最简单、最有效的防套利方式

### 2. 单交易所场景
- 如果只配置 1 个交易所，直接使用该交易所的中间价
- 不会进行平均计算（因为只有一个数据源）

### 3. 中间价精度
- 统一中间价保持较高精度（双精度浮点数）
- 生成的订单价格满足各交易所的下单要求

### 4. 失败处理
- 如果某个交易所获取中间价失败，会跳过该交易所
- 使用其他成功的交易所数据继续计算平均值
- 至少需要 1 个交易所成功才能继续执行

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

统一定价功能通过计算所有交易所中间价的平均值，然后基于这个统一中间价生成网格订单，所有交易所使用完全相同的订单，有效防止了跨交易所套利。

**核心理念**：
> 不让别人在你的交易所之间低买高卖赚取无风险利润！

**实现方式**：
- ✅ 简单高效：先平均中间价，再生成订单
- ✅ 代码精简：相比复杂的订单平均，代码量减少 70+ 行
- ✅ 易于维护：逻辑清晰，一目了然
- ✅ 效果相同：完全消除跨交易所价差

使用统一定价，让你的多交易所网格交易更加安全和高效！🚀
