# 多交易所网格交易机器人 (Multi-Exchange Grid Trading Bot)

一个用 Rust 编写的自动化网格交易机器人，支持多个主流加密货币交易所进行智能布单。

## 支持的交易所

- ✅ **MEXC** - 完整支持（批量订单 API）
- ✅ **Gate.io** - 完整支持（批量订单 API）
- ✅ **KuCoin** - 完整支持（批量订单 API）

## 功能特点

- 🔌 **多交易所支持** - 单个配置文件同时管理多个交易所账户，使用统一网格参数
- 🚀 **批量订单 API** - 使用交易所原生批量下单接口，显著提升下单速度
- 📊 **智能网格布单** - 在当前价格附近自动计算并放置网格订单
- 📈 **动态订单深度** - 根据距离当前价格的远近调整订单深度：
  - 越靠近当前价格，订单深度越小（快速成交）
  - 离当前价格越远，订单深度越大（提供流动性）
- ⚙️ **灵活配置** - 可配置的网格参数：
  - 买单和卖单的价格范围（百分比）
  - 网格间隔（百分比）
  - 总买入和卖出价值
  - 网格层数
- 📋 **订单管理** - 查询挂单、历史订单和交易记录
- 📸 **账户快照和盈亏追踪** - 自动记录每次布单后的账户状态，实时分析盈亏
- 🔄 **Watch 模式** - 持续监控模式，定期自动检查并布单，记录账户变化
- 🎨 **实时监控仪表板** - REST API + React 前端，提供实时数据可视化
  - 账户余额、市场深度、订单深度
  - 盈亏分析、快照历史
  - 自动刷新（每30秒）
  - 响应式设计，支持移动设备
- ⚡ **高性能** - 使用 Rust 异步编程，批量 API 调用

## 快速开始

```bash
# 1. 克隆项目
git clone <repository-url>
cd mm

# 2. 编译项目
cargo build --release

# 3. 创建配置文件
cargo run --release -- create-config config.json

# 4. 编辑配置文件，填入你的 API 密钥和网格参数
vim config.json

# 5. 执行网格布单
cargo run --release -- place config.json
```

## 安装

确保你已经安装了 Rust：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

克隆项目并构建：

```bash
cargo build --release
```

## 配置

### 1. 创建配置文件

```bash
cargo run -- --create-config config.json
```

这将创建一个示例配置文件 `config.json`。

### 2. 编辑配置文件

打开 `config.json` 并填入你的配置。配置文件支持多个交易所，所有交易所共享相同的网格参数：

#### 多交易所配置示例

```json
{
  "exchanges": [
    {
      "name": "mexc",
      "api_key": "your_mexc_api_key",
      "api_secret": "your_mexc_api_secret"
    },
    {
      "name": "gate",
      "api_key": "your_gate_api_key",
      "api_secret": "your_gate_api_secret"
    },
    {
      "name": "kucoin",
      "api_key": "your_kucoin_api_key",
      "api_secret": "your_kucoin_api_secret",
      "api_passphrase": "your_kucoin_api_passphrase"
    }
  ],
  "grid": {
    "symbol": "BTCUSDT",
    "buy_price_percentage": 5.0,
    "sell_price_percentage": 5.0,
    "grid_interval_percentage": 0.5,
    "total_buy_value": 100.0,
    "total_sell_value": 100.0,
    "grid_levels": 10
  }
}
```

**注意：**
- 你可以配置一个或多个交易所
- 所有交易所将使用相同的网格参数（`grid` 配置）
- 每个交易所独立执行布单和查询操作
- KuCoin 需要额外的 `api_passphrase` 字段

#### 配置参数说明

**交易所配置（exchanges 数组）：**
- `name`: 交易所名称（"mexc" | "gate" | "kucoin"）
- `api_key`: 交易所 API Key
- `api_secret`: 交易所 API Secret
- `api_passphrase`: API 密码短语（仅 KuCoin 需要）

**网格参数：**
- `symbol`: 交易对（如 BTCUSDT）
- `buy_price_percentage`: 买单价格范围（相对当前价格的百分比，如 5.0 表示在当前价格下方 5% 范围内布单）
- `sell_price_percentage`: 卖单价格范围（相对当前价格的百分比）
- `grid_interval_percentage`: 每个网格的间隔（百分比，如 0.5 表示每个网格间隔 0.5%）
- `total_buy_value`: 买单总价值（USDT）
- `total_sell_value`: 卖单对应的币数总价值（USDT）
- `grid_levels`: 网格层数
- `minimal_order_value`: 最小订单总价值，低于此值不布单

**中间价计算参数（可选）：**
- `mid_price_method_bid`: Bid 价格计算方法（默认 `"simple"`），可选：
  - `"simple"`: 直接取最优买价
  - `"weighted_by_volume"`: 按订单量加权平均
  - `"volume_threshold"`: 基于最近成交买单价值的深度阈值（推荐）
- `mid_price_method_ask`: Ask 价格计算方法（默认 `"simple"`），可选：
  - `"simple"`: 直接取最优卖价
  - `"weighted_by_volume"`: 按订单量加权平均
  - `"volume_threshold"`: 基于阈值（较少使用）
- `orderbook_depth`: 获取的订单簿深度档位（默认 20）
- `volume_threshold_usdt`: VolumeThreshold 方法的默认阈值，单位 USDT（默认 100.0）
  - 当没有成交历史时使用此阈值
  - 可根据交易对流动性调整，主流币可设置更大值（如 500-1000）

详细说明请参考：[PRICE_CALCULATION.md](PRICE_CALCULATION.md)

### 获取交易所 API 密钥

#### MEXC
1. 登录 [MEXC](https://www.mexc.com/)
2. 进入 API 管理
3. 创建新的 API 密钥
4. 确保启用现货交易权限
5. 记录 API Key 和 Secret Key

#### Gate.io
1. 登录 [Gate.io](https://www.gate.io/)
2. 进入个人中心 -> API 管理
3. 创建新的 API 密钥
4. 确保启用现货交易权限
5. 记录 API Key 和 Secret Key

#### KuCoin
1. 登录 [KuCoin](https://www.kucoin.com/)
2. 进入 API 管理
3. 创建新的 API 密钥
4. 设置 API 密码短语（Passphrase）
5. 确保启用现货交易权限
6. 记录 API Key、Secret Key 和 Passphrase

## 使用

程序提供了多个命令来管理你的网格交易：

### 查看帮助

```bash
cargo run --release -- --help
```

或使用编译后的二进制文件：

```bash
./target/release/mexc-grid-trader --help
```

### 1. 创建配置文件

```bash
cargo run --release -- create-config [配置文件路径]
```

示例：
```bash
cargo run --release -- create-config my_config.json
```

### 2. 执行网格布单

```bash
cargo run --release -- place [配置文件路径]
```

示例：
```bash
cargo run --release -- place config.json
```

运行流程：
1. 程序会读取配置文件
2. 遍历所有配置的交易所
3. 对每个交易所：
   - 获取指定交易对的当前价格
   - 根据配置计算所有网格订单
   - 显示订单预览（包括价格、数量、价值）
   - 询问是否执行下单（首个交易所）
   - 如果确认，使用批量订单 API 快速下单
   - 显示下单结果统计
4. 所有交易所处理完成后显示总体结果

**性能优化：**
- 程序使用各交易所的批量订单 API，显著提升下单速度
- MEXC & Gate.io: 所有订单一次性提交
- KuCoin: 每批最多 5 个订单，自动分批提交
- 相比逐个下单，速度提升 5-10 倍

### 3. 查询当前挂单

查询所有配置交易所的挂单：

```bash
cargo run --release -- orders [配置文件路径]
```

程序会遍历配置文件中的所有交易所，分别显示每个交易所的挂单情况。

输出示例：
```
Fetching open orders...

Open Orders (20):
Symbol          Side       Price        Quantity     Filled       Status
--------------------------------------------------------------------------------
BTCUSDT         BUY            49750.00      0.00006      0.00000 NEW
BTCUSDT         BUY            49500.00      0.00022      0.00000 NEW
BTCUSDT         SELL           50250.00      0.00006      0.00000 NEW
...
--------------------------------------------------------------------------------
Total unfilled value: 1000.00 USDT
```

### 4. 查询历史交易记录

查询所有配置交易所最近 50 条交易记录（默认）：

```bash
cargo run --release -- trades [配置文件路径]
```

指定返回的交易数量：

```bash
cargo run --release -- trades --limit 100
```

或简写：

```bash
cargo run --release -- trades -l 100
```

程序会遍历配置文件中的所有交易所，分别显示每个交易所的交易记录。

输出示例：
```
Fetching trades for BTCUSDT...

Trade History (50):
Trade ID     Side     Price        Quantity     Total        Fee
--------------------------------------------------------------------------------
123456789    BUY            49750.00      0.00010      4.98      0.00010 USDT
123456790    SELL           50250.00      0.00010      5.03      0.00010 USDT
...
--------------------------------------------------------------------------------
Total BUY value:  497.50 USDT
Total SELL value: 502.50 USDT
Total fees:       0.01000
Net profit/loss:  5.00 USDT (1.00%)
```

### 5. 取消所有挂单

```bash
cargo run --release -- cancel [配置文件路径]
```

强制取消（不需要确认）：

```bash
cargo run --release -- cancel --force
```

### 6. 持续布单模式（Watch）

启动持续监控模式，定期自动检查并布单：

```bash
cargo run --release -- watch [配置文件路径]
```

自定义检查间隔（默认 120 秒）：

```bash
cargo run --release -- watch --interval 300  # 每5分钟检查一次
```

或简写：

```bash
cargo run --release -- watch -i 300
```

Watch 模式特点：
- 🔄 定期自动检查市场并布单
- 📸 每次布单后自动记录账户快照
- 📊 实时显示账户变化和盈亏
- 💾 所有快照保存到 `.account_snapshots_{交易所}_{交易对}.jsonl` 文件
- ⏸️ 按 Ctrl+C 可停止运行

### 7. 实时监控仪表板（Report Dashboard）

启动 REST API 服务器和 React 前端，提供实时数据可视化：

```bash
# 快速启动（推荐）
./start_dashboard.sh

# 或手动启动后端
cargo run --release -- report [配置文件路径]

# 自定义端口
cargo run --release -- report --port 8080
```

仪表板功能：
- 📊 **账户余额** - 实时显示各资产的可用、锁定和总余额
- 📈 **市场深度** - 当前价格和各价格区间的订单簿深度（±0.5%, ±1%, ±2%, ±5%, ±10%）
- 📋 **我的订单深度** - 你的挂单统计（数量、价格范围、总价值），不显示具体订单
- 📸 **快照历史** - 账户快照数量和最新快照信息
- 💹 **盈亏统计** - 时间段内的收益变化和百分比
- 🔄 **自动刷新** - 每30秒自动更新数据
- 📱 **响应式设计** - 支持桌面和移动设备

访问地址：
- **后端 API**: `http://localhost:3000/api/report`
- **健康检查**: `http://localhost:3000/health`
- **前端界面**: `http://localhost:3000` (或下一个可用端口)

详细使用说明请参考：[REPORT_DASHBOARD.md](REPORT_DASHBOARD.md)

输出示例：
```
╔════════════════════════════════════════════════════════╗
║              LATEST ACCOUNT STATUS                     ║
╚════════════════════════════════════════════════════════╝

📅 Timestamp: 2025-01-15 10:30:45 UTC
🔄 Iteration: #25
💹 Mid Price: 50000.123456

🪙 Assets:
  🪙 BTC       Free:      0.02000000  Locked:      0.00500000  Total:      0.02500000
  💵 USDT      Free:   1250.50000000  Locked:    250.00000000  Total:   1500.50000000

💰 Total Value: 2751.00 USDT

╔════════════════════════════════════════════════════════╗
║            DETAILED PERIOD ANALYSIS                    ║
╚════════════════════════════════════════════════════════╝

📊 Overall Statistics:
  Total Snapshots: 25
  Total Iterations: 24
  Profitable Iterations: 18 (75.0%)

💰 Overall Performance:
  Initial Value: 2500.00 USDT
  Final Value:   2751.00 USDT
  📈 Total P&L:    251.00 USDT (+10.04%)
  Average per iteration: 10.46 USDT

📈 Recent Iterations:
  ✅ Iteration #21: +12.50 USDT (+0.46%)
  ✅ Iteration #22: +8.30 USDT (+0.30%)
  ❌ Iteration #23: -3.20 USDT (-0.12%)
  ✅ Iteration #24: +15.60 USDT (+0.57%)
```

## 技术架构

### 交易所抽象层

本项目使用 Rust trait 系统实现了统一的交易所接口，使得添加新交易所非常简单：

```rust
#[async_trait]
pub trait Exchange: Send + Sync {
    async fn get_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook>;
    async fn get_mid_price(&self, symbol: &str) -> Result<f64>;
    async fn place_batch_limit_orders(&self, orders: Vec<BatchOrder>) -> Result<Vec<Result<OrderResponse>>>;
    // ... 更多方法
}
```

### 批量订单实现

各交易所的批量订单 API 实现细节：

| 交易所 | API 端点 | 签名算法 | 批量限制 |
|-------|---------|---------|---------|
| MEXC | `/api/v3/batchOrders` | HMAC-SHA256 (hex) | 无限制 |
| Gate.io | `/api/v4/spot/batch_orders` | HMAC-SHA512 (hex) | 无限制 |
| KuCoin | `/api/v1/orders/multi` | HMAC-SHA256 (base64) | 每批 5 个 |

### 交易对格式转换

不同交易所使用不同的交易对格式，程序会自动转换：

- **内部格式**: `BTCUSDT`
- **MEXC**: `BTCUSDT` (无需转换)
- **Gate.io**: `BTC_USDT` (下划线分隔)
- **KuCoin**: `BTC-USDT` (连字符分隔)

## 订单深度算法

订单深度采用二次方加权算法：

- 距离当前价格越近的订单，权重越小，深度越浅
- 距离当前价格越远的订单，权重越大，深度越深
- 权重计算公式：`weight = (level / total_levels)^2`

这样可以确保：
- 靠近当前价格的订单快速成交
- 远离当前价格的订单提供更大的流动性

## 状态文件管理

程序会为每个交易所单独保存两类状态文件：

### 1. 价格状态文件
- 格式：`.state_{交易所名称}_{交易对}.json`
- 例如：`.state_mexc_BTCUSDT.json`、`.state_gate_BTCUSDT.json`
- 用途：跟踪上次布单的价格，实现价格下跌时自动取消买单

### 2. 账户快照文件
- 格式：`.account_snapshots_{交易所名称}_{交易对}.jsonl`
- 例如：`.account_snapshots_mexc_BTCUSDT.jsonl`
- 用途：记录每次布单后的账户状态（余额、价格、时间等）
- 格式：每行一个 JSON 对象（JSONL 格式）
- 内容包括：
  - 时间戳和人类可读的日期时间
  - 交易所名称和交易对
  - 各资产的余额（可用、冻结、总计）
  - 当时的中间价
  - Watch 模式的迭代次数

这些快照数据用于：
- 追踪账户余额变化
- 分析交易策略的盈亏
- 生成详细的统计报告
- 评估网格交易的整体表现

## 示例

假设当前 BTC 价格为 50,000 USDT，配置如下：

```json
{
  "buy_price_percentage": 5.0,
  "sell_price_percentage": 5.0,
  "grid_interval_percentage": 0.5,
  "total_buy_value": 1000.0,
  "total_sell_value": 1000.0,
  "grid_levels": 10
}
```

程序将：
- 在 47,500 - 50,000 USDT 之间布置 10 个买单
- 在 50,000 - 52,500 USDT 之间布置 10 个卖单
- 总买单价值约 1000 USDT
- 总卖单币数约价值 1000 USDT
- 越靠近 50,000 的订单深度越小

## 注意事项

⚠️ **风险警告**

- 本程序用于教育和研究目的
- 加密货币交易存在高风险，可能导致资金损失
- 使用前请充分测试，建议先在测试网或小金额测试
- 确保你完全理解网格交易的原理和风险
- 请妥善保管你的 API 密钥，不要泄露给他人
- 建议 API 密钥只开启现货交易权限，不要开启提现权限

## 安全建议

1. **API 密钥管理**
   - 不要在配置文件中硬编码 API 密钥
   - 使用环境变量或密钥管理服务
   - 定期更换 API 密钥
   - 不要将配置文件提交到版本控制系统

2. **API 权限设置**
   - 只开启现货交易权限
   - 不要开启提现权限
   - 不要开启合约交易权限（如不需要）

3. **网络安全**
   - 设置 API 的 IP 白名单（所有交易所都支持）
   - 使用 VPN 或固定 IP 以提高安全性

4. **监控和审计**
   - 定期监控账户活动
   - 检查异常交易和登录记录
   - 设置交易所的邮件/短信通知

5. **多交易所使用**
   - 为不同交易所使用不同的 API 密钥
   - 分散风险，不要将所有资金放在一个交易所
   - 定期备份配置文件

## 开发

### 运行测试

```bash
cargo test
```

### 检查代码

```bash
cargo clippy
```

### 格式化代码

```bash
cargo fmt
```

### 添加新的交易所

要添加新的交易所支持，只需：

1. 在 `src/exchanges/` 目录创建新文件（如 `binance.rs`）
2. 实现 `Exchange` trait
3. 在 `src/exchanges/mod.rs` 中添加导出
4. 在 `ExchangeType` 枚举中添加新类型
5. 在 `create_exchange()` 函数中添加创建逻辑

详细说明请参考 [MULTI_EXCHANGE.md](MULTI_EXCHANGE.md)。

### 项目结构

```
src/
├── main.rs              # 主程序入口和命令行接口
├── config.rs            # 配置文件处理
├── exchange.rs          # Exchange trait 定义
├── exchanges/
│   ├── mod.rs          # 交易所模块导出和工厂函数
│   ├── mexc.rs         # MEXC 实现
│   ├── gate.rs         # Gate.io 实现
│   └── kucoin.rs       # KuCoin 实现
├── order_calculator.rs  # 网格订单计算逻辑
├── price_calculator.rs  # 中间价计算逻辑
├── state.rs            # 交易状态管理
└── account_snapshot.rs  # 账户快照和盈亏分析
```

## 许可证

MIT License

## 免责声明

本软件按"原样"提供，不提供任何明示或暗示的保证。作者不对使用本软件造成的任何损失负责。使用者需自行承担所有风险。
