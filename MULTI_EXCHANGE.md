# 多交易所支持

本程序使用 Rust trait 架构支持多个加密货币交易所。

## 支持的交易所

| 交易所 | 状态 | 说明 |
|--------|------|------|
| MEXC | ✅ 完整实现 | 所有功能都已实现和测试 |
| Gate.io | 🚧 部分实现 | 仅实现订单簿和中间价获取 |
| KuCoin | 🚧 部分实现 | 仅实现订单簿和中间价获取 |

## 配置方式

### MEXC 配置

```json
{
  "exchange": "mexc",
  "api_key": "your_mexc_api_key",
  "api_secret": "your_mexc_api_secret",
  "grid": { ... }
}
```

### Gate.io 配置

```json
{
  "exchange": "gate",
  "api_key": "your_gate_api_key",
  "api_secret": "your_gate_api_secret",
  "grid": { ... }
}
```

**注意**：Gate.io 使用不同的交易对格式：
- 程序内部：`BTCUSDT`
- Gate.io API：`BTC_USDT` (自动转换)

### KuCoin 配置

```json
{
  "exchange": "kucoin",
  "api_key": "your_kucoin_api_key",
  "api_secret": "your_kucoin_api_secret",
  "api_passphrase": "your_kucoin_api_passphrase",
  "grid": { ... }
}
```

**注意**：
- KuCoin 需要额外的 `api_passphrase` 字段
- KuCoin 使用格式：`BTC-USDT` (自动转换)

## 架构说明

### Exchange Trait

所有交易所都实现统一的 `Exchange` trait:

```rust
#[async_trait]
pub trait Exchange: Send + Sync {
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook>;
    async fn get_mid_price(&self, symbol: &str) -> Result<f64>;
    async fn place_limit_order(&self, symbol: &str, side: &str, quantity: f64, price: f64) -> Result<OrderResponse>;
    async fn get_account_info(&self) -> Result<AccountInfo>;
    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<OpenOrder>>;
    async fn get_all_orders(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<OpenOrder>>;
    async fn get_my_trades(&self, symbol: &str, limit: Option<u32>) -> Result<Vec<Trade>>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<serde_json::Value>;
    fn get_symbol_assets(&self, symbol: &str) -> (String, String);
}
```

### 代码结构

```
src/
├── exchange.rs           # Exchange trait 定义和通用数据结构
├── exchanges/
│   ├── mod.rs           # 交易所模块导出和工厂函数
│   ├── mexc.rs          # MEXC 实现 (完整)
│   ├── gate.rs          # Gate.io 实现 (部分)
│   └── kucoin.rs        # KuCoin 实现 (部分)
└── main.rs              # 使用 Arc<dyn Exchange> 的主程序
```

### 工厂模式

使用工厂函数创建交易所实例：

```rust
let client = create_exchange(
    &exchange_type,
    api_key,
    api_secret,
    api_passphrase,  // 仅 KuCoin 需要
)?;
```

返回 `Arc<dyn Exchange>`，支持动态分发。

## 添加新交易所

要添加新的交易所支持，按以下步骤操作：

### 1. 创建交易所模块

在 `src/exchanges/` 下创建新文件，例如 `binance.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use crate::exchange::*;

#[derive(Debug, Clone)]
pub struct BinanceExchange {
    api_key: String,
    api_secret: String,
    client: reqwest::Client,
}

impl BinanceExchange {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Exchange for BinanceExchange {
    // 实现所有 trait 方法
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        // 实现代码
    }

    // ... 其他方法
}
```

### 2. 更新 mod.rs

在 `src/exchanges/mod.rs` 添加：

```rust
pub mod binance;
pub use binance::BinanceExchange;

pub enum ExchangeType {
    Mexc,
    Gate,
    Kucoin,
    Binance,  // 新增
}

pub fn create_exchange(...) -> Result<Arc<dyn Exchange>> {
    match exchange_type {
        // ...
        ExchangeType::Binance => {
            Ok(Arc::new(BinanceExchange::new(api_key, api_secret)))
        }
    }
}
```

### 3. 测试

创建配置文件测试新交易所：

```json
{
  "exchange": "binance",
  "api_key": "...",
  "api_secret": "...",
  "grid": { ... }
}
```

## 当前限制

### Gate.io 和 KuCoin

目前只实现了以下功能：
- ✅ 获取订单簿
- ✅ 获取中间价
- ❌ 下单 (返回 "not yet complete" 错误)
- ❌ 查询账户
- ❌ 查询订单
- ❌ 查询交易
- ❌ 取消订单

要完整实现这些交易所，需要：
1. 查阅对应交易所的 API 文档
2. 实现认证签名算法
3. 实现各个 API 端点的请求和响应解析
4. 处理交易对格式转换
5. 测试所有功能

## API 签名算法差异

不同交易所使用不同的签名算法：

| 交易所 | 签名算法 | 编码方式 |
|--------|----------|----------|
| MEXC | HMAC-SHA256 | Hex |
| Gate.io | HMAC-SHA512 | Hex |
| KuCoin | HMAC-SHA256 | Base64 |

这些差异都已在各自的实现中处理。

## 贡献

欢迎贡献代码完善 Gate.io 和 KuCoin 的实现，或添加新的交易所支持！
