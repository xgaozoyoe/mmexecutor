# 中间价计算方法说明

## 概述

为了更准确地计算网格布单的基准价格，系统提供了三种中间价计算方法。不同的方法适合不同的市场情况和交易策略。

## 三种计算方法

### 1. Simple - 简单平均法

**原理：** 取订单簿最优买价和最优卖价的简单平均值

**公式：**
```
mid_price = (best_bid + best_ask) / 2
```

**优点：**
- 计算最简单快速
- 适合流动性好、深度充足的市场

**缺点：**
- 忽略订单深度
- 当最优买卖盘口深度很小时，价格可能不准确

**适用场景：**
- 主流交易对（如 BTC/USDT）
- 流动性充足的市场
- 需要快速决策的场景

### 2. WeightedByVolume - 量加权平均法 ⭐ 推荐

**原理：** 根据订单簿各档位的订单量进行加权平均

**公式：**
```
weighted_price = Σ(price × quantity) / Σ(quantity)
mid_price = (weighted_bid_price + weighted_ask_price) / 2
```

**优点：**
- 考虑了订单深度
- 更能反映真实的市场价格中心
- 不容易被小额订单干扰

**缺点：**
- 需要获取较多档位的订单簿（建议 20 档）
- 计算稍复杂

**适用场景：**
- 推荐作为默认方法
- 适合大多数交易场景
- 深度一般的交易对

**配置示例：**
```json
{
  "grid": {
    "mid_price_method": "weighted_by_volume",
    "orderbook_depth": 20
  }
}
```

### 3. VolumeThreshold - 动态深度阈值法 🎯 智能

**原理：** 基于我们之前布置的买单中已成交订单的价值，在订单簿中找到累计深度达到该价值的价格

**工作流程：**
1. 获取最近 500 笔历史订单（包括已成交和未成交）
2. 筛选出买单（BUY）中已经成交的订单（`executed_qty > 0`）
3. 统计这些已成交买单的总价值 V
4. 在订单簿买卖盘中，找到累计深度达到 V 的价格档位
5. 取两侧价格的平均值

**优点：**
- 动态适应我们自己的实际成交情况
- 考虑了我们实际需要的市场深度
- 能够反映我们订单的实际可成交区间
- 避免被市场假墙误导

**缺点：**
- 需要获取历史订单（部分交易所可能有限制）
- 如果之前没有买单成交，会使用默认阈值（100 USDT）
- 首次运行时没有历史数据

**适用场景：**
- 已经运行过一段时间，有成交历史
- 希望价格更贴近我们自己的成交价
- 需要根据实际成交调整网格布局

**配置示例：**
```json
{
  "grid": {
    "mid_price_method": "volume_threshold",
    "orderbook_depth": 20
  }
}
```

## 配置参数说明

### mid_price_method

中间价计算方法，可选值：
- `"simple"` - 简单平均法
- `"weighted_by_volume"` - 量加权平均法（推荐）
- `"volume_threshold"` - 动态深度阈值法

### orderbook_depth

获取订单簿的深度档位数量。

**建议值：**
- Simple 方法：1-5 档即可
- WeightedByVolume 方法：20 档
- VolumeThreshold 方法：20 档

**注意：** 不同交易所对订单簿深度的限制不同：
- MEXC: 支持 5, 10, 20, 50, 100 档
- Gate.io: 最多 100 档
- KuCoin: 支持 20, 100 档

## 配置示例

### 完整配置示例（推荐）

```json
{
  "exchange": "mexc",
  "api_key": "your_api_key",
  "api_secret": "your_api_secret",
  "grid": {
    "symbol": "BTCUSDT",
    "first_buy_offset_percentage": 0.5,
    "first_sell_offset_percentage": 0.5,
    "buy_price_percentage": 5.0,
    "sell_price_percentage": 5.0,
    "grid_interval_percentage": 0.5,
    "total_buy_value": 500.0,
    "total_sell_value": 500.0,
    "grid_levels": 10,
    "minimal_order_value": 10.0,

    "mid_price_method": "weighted_by_volume",
    "orderbook_depth": 20
  }
}
```

### 使用 VolumeThreshold 方法

```json
{
  "grid": {
    "mid_price_method": "volume_threshold",
    "orderbook_depth": 20
  }
}
```

## 运行时输出示例

### Simple 方法
```
📊 Price Calculation (Simple method):
  Simple average of best bid and ask
  Bid: 95123.450000, Ask: 95125.320000
  Mid Price:   95124.385000
```

### WeightedByVolume 方法
```
📊 Price Calculation (WeightedByVolume method):
  Volume-weighted average
  Weighted Bid: 95121.234567 (vol: 12.45), Weighted Ask: 95126.789012 (vol: 15.32)
  Mid Price:   95124.011789
```

### VolumeThreshold 方法
```
Fetching recent trades to calculate volume threshold...

📊 Price Calculation (VolumeThreshold method):
  Price at 2345.67 USDT volume threshold (from recent trades)
  Bid at 2500.00 USDT: 95118.500000, Ask at 2500.00 USDT: 95130.200000
  Mid Price:   95124.350000
```

## 选择建议

| 场景 | 推荐方法 | 理由 |
|------|---------|------|
| 主流币种（BTC/ETH/USDT） | WeightedByVolume | 深度充足，加权更准确 |
| 小市值币种 | VolumeThreshold | 深度不稳定，跟随实际成交 |
| 高频测试 | Simple | 快速决策 |
| 流动性差的交易对 | VolumeThreshold | 避免被假墙误导 |
| 默认推荐 | WeightedByVolume | 平衡准确性和稳定性 |

## 常见问题

### Q1: VolumeThreshold 方法获取成交记录失败怎么办？

**A:** 系统会自动降级使用默认阈值（1000 USDT），并显示警告信息：
```
⚠️  Warning: Failed to fetch trades: XXX. Using default threshold.
```

### Q2: orderbook_depth 设置多少合适？

**A:**
- Simple 方法：1 档即可
- 其他方法：建议 20 档，最多不超过 50 档
- 深度越大，数据传输越慢，但更准确

### Q3: 三种方法计算出的价格差异大吗？

**A:**
- 在流动性好的市场，差异通常小于 0.1%
- 在流动性差的市场，差异可能达到 0.5%-1%
- VolumeThreshold 在大额订单挂单时差异可能更大

### Q4: 如何验证计算结果是否合理？

**A:**
1. 查看输出的详细信息
2. 对比交易所界面的盘口价格
3. 使用 Simple 方法作为基准对比
4. 观察实际布单价格是否合理

## 技术实现

核心代码位于 `src/price_calculator.rs`，实现了 `PriceCalculator` 结构体：

```rust
pub struct PriceCalculator;

impl PriceCalculator {
    pub fn calculate_mid_price(
        order_book: &OrderBook,
        method: &MidPriceMethod,
        recent_trades: Option<&[Trade]>,
    ) -> Result<MidPriceResult>
}
```

返回结果包含：
- `mid_price`: 计算出的中间价
- `bid_price`: 参考的买价
- `ask_price`: 参考的卖价
- `method_description`: 方法描述
- `details`: 详细计算信息
