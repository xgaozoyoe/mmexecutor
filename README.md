# MEXC Grid Trading Bot

一个用 Rust 编写的自动化网格交易机器人，可以连接 MEXC 交易所进行智能布单。

## 功能特点

- 连接 MEXC 交易所 API
- 在当前价格附近进行智能网格布单
- 根据距离当前价格的远近动态调整订单深度：
  - 越靠近当前价格，订单深度越小
  - 离当前价格越远，订单深度越大
- 可配置的网格参数：
  - 买单和卖单的价格范围（百分比）
  - 网格间隔（百分比）
  - 总买入和卖出价值
  - 网格层数

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

打开 `config.json` 并填入你的配置：

```json
{
  "api_key": "your_mexc_api_key",
  "api_secret": "your_mexc_api_secret",
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

#### 配置参数说明

- `api_key`: MEXC API Key
- `api_secret`: MEXC API Secret
- `symbol`: 交易对（如 BTCUSDT）
- `buy_price_percentage`: 买单价格范围（相对当前价格的百分比，如 5.0 表示在当前价格下方 5% 范围内布单）
- `sell_price_percentage`: 卖单价格范围（相对当前价格的百分比）
- `grid_interval_percentage`: 每个网格的间隔（百分比，如 0.5 表示每个网格间隔 0.5%）
- `total_buy_value`: 买单总价值（USDT）
- `total_sell_value`: 卖单对应的币数总价值（USDT）
- `grid_levels`: 网格层数

### 获取 MEXC API 密钥

1. 登录 [MEXC](https://www.mexc.com/)
2. 进入 API 管理
3. 创建新的 API 密钥
4. 确保启用现货交易权限
5. 记录 API Key 和 Secret Key

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
2. 获取指定交易对的当前价格
3. 根据配置计算所有网格订单
4. 显示订单预览（包括价格、数量、价值）
5. 询问是否执行下单
6. 如果确认，开始逐个下单
7. 显示下单结果统计

### 3. 查询当前挂单

查询配置文件中指定交易对的挂单：

```bash
cargo run --release -- orders [配置文件路径]
```

查询所有交易对的挂单：

```bash
cargo run --release -- orders --all
```

或简写：

```bash
cargo run --release -- orders -a
```

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

查询最近 50 条交易记录（默认）：

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

## 订单深度算法

订单深度采用二次方加权算法：

- 距离当前价格越近的订单，权重越小，深度越浅
- 距离当前价格越远的订单，权重越大，深度越深
- 权重计算公式：`weight = (level / total_levels)^2`

这样可以确保：
- 靠近当前价格的订单快速成交
- 远离当前价格的订单提供更大的流动性

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

1. 不要在配置文件中硬编码 API 密钥
2. 使用环境变量或密钥管理服务
3. 定期更换 API 密钥
4. 监控账户活动
5. 设置 API 的 IP 白名单

## 开发

运行测试：

```bash
cargo test
```

检查代码：

```bash
cargo clippy
```

格式化代码：

```bash
cargo fmt
```

## 许可证

MIT License

## 免责声明

本软件按"原样"提供，不提供任何明示或暗示的保证。作者不对使用本软件造成的任何损失负责。使用者需自行承担所有风险。
