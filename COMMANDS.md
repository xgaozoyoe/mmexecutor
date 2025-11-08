# 命令速查表

## 快速参考

### 查看所有命令
```bash
mexc-grid-trader --help
```

### 创建配置文件
```bash
mexc-grid-trader create-config [配置文件路径]
```

### 执行网格布单
```bash
mexc-grid-trader place [配置文件路径]
```

### 查询当前挂单
```bash
# 查询配置文件中指定交易对的挂单
mexc-grid-trader orders [配置文件路径]

# 查询所有交易对的挂单
mexc-grid-trader orders --all
mexc-grid-trader orders -a
```

### 查询历史交易
```bash
# 查询最近 50 条交易（默认）
mexc-grid-trader trades [配置文件路径]

# 指定返回数量
mexc-grid-trader trades --limit 100
mexc-grid-trader trades -l 100
```

## 使用示例

### 完整工作流程

1. **创建配置文件**
   ```bash
   mexc-grid-trader create-config my_btc_config.json
   ```

2. **编辑配置文件**
   编辑 `my_btc_config.json`，填入 API 密钥和交易参数

3. **执行布单**
   ```bash
   mexc-grid-trader place my_btc_config.json
   ```

4. **监控挂单**
   ```bash
   mexc-grid-trader orders my_btc_config.json
   ```

5. **查看交易历史**
   ```bash
   mexc-grid-trader trades my_btc_config.json
   ```

### 多交易对管理

为不同的交易对创建不同的配置文件：

```bash
# BTC 配置
mexc-grid-trader create-config btc_config.json
mexc-grid-trader place btc_config.json

# ETH 配置
mexc-grid-trader create-config eth_config.json
mexc-grid-trader place eth_config.json

# 查询所有挂单
mexc-grid-trader orders btc_config.json --all
```

## 命令行参数

### create-config
```
创建示例配置文件

USAGE:
    mexc-grid-trader create-config [PATH]

ARGS:
    <PATH>    配置文件路径 [default: config.json]
```

### place
```
执行网格布单

USAGE:
    mexc-grid-trader place [CONFIG]

ARGS:
    <CONFIG>    配置文件路径 [default: config.json]
```

### orders
```
查询当前挂单

USAGE:
    mexc-grid-trader orders [OPTIONS] [CONFIG]

ARGS:
    <CONFIG>    配置文件路径 [default: config.json]

OPTIONS:
    -a, --all    是否显示所有交易对的挂单
    -h, --help   打印帮助信息
```

### trades
```
查询历史交易记录

USAGE:
    mexc-grid-trader trades [OPTIONS] [CONFIG]

ARGS:
    <CONFIG>    配置文件路径 [default: config.json]

OPTIONS:
    -l, --limit <LIMIT>    返回的交易记录数量 [default: 50]
    -h, --help             打印帮助信息
```

## 技巧

### 使用别名简化命令

在 `~/.bashrc` 或 `~/.zshrc` 中添加：

```bash
alias mgt='./target/release/mexc-grid-trader'
```

然后就可以使用简短命令：

```bash
mgt orders
mgt trades -l 100
mgt place my_config.json
```

### 使用默认配置文件

如果配置文件名为 `config.json` 且在当前目录，可以省略配置文件参数：

```bash
mgt orders       # 使用默认 config.json
mgt trades       # 使用默认 config.json
mgt place        # 使用默认 config.json
```

### 自动化脚本示例

创建一个监控脚本 `monitor.sh`：

```bash
#!/bin/bash
while true; do
    clear
    echo "=== 当前挂单 ==="
    mexc-grid-trader orders config.json
    echo ""
    echo "=== 最近交易 ==="
    mexc-grid-trader trades config.json -l 10
    sleep 60
done
```

运行：
```bash
chmod +x monitor.sh
./monitor.sh
```
