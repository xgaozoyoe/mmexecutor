# 如何查看平均交易价格

## 🎯 快速开始

### 1️⃣ 启动 Report 服务
```bash
cd /Users/xingao/mmexecutor
./target/release/mexc-grid-trader report --config config.json
```

### 2️⃣ 等待数据收集
服务会每 30 秒自动保存账户快照。你需要：
- ⏱️ 等待至少 **1 分钟**（2 个快照）
- 💰 确保期间有**交易发生**（余额有变化）

### 3️⃣ 查看前端

**方法 A：开发模式（推荐）**
```bash
cd frontend
npm start
```
浏览器会自动打开 `http://localhost:3000` 或 `http://localhost:3001`

**方法 B：生产模式**
```bash
cd frontend
npm run build
npx serve -s build -p 8080
```
然后访问 `http://localhost:8080`

### 4️⃣ 找到平均价格卡片
在页面中向下滚动到 **"💹 Profit & Loss Summary"** 部分，你会看到：

```
┌─────────────────────────────────────────────────┐
│   📊 Average Trading Price Analysis             │
├─────────────────────────────────────────────────┤
│   Direction: Buying ZKWASM                      │
│   Average Price: 0.015050 USDT                  │
│                                                  │
│   Calculation: 150.50 ÷ 10000.00000000         │
│                                                  │
│   Current Market Price: 0.015200                │
│   Price Difference: -0.000150 (-0.99%)          │
│                                                  │
│   ✅ Bought below current market price         │
└─────────────────────────────────────────────────┘
```

## ❓ 为什么看不到？

### 问题 1: 没有 PnL Summary 卡片
**原因：** 快照数据不足

**解决方案：**
```bash
# 检查快照文件
ls -lh .snapshots_*

# 查看快照数量
wc -l .snapshots_gate_ZKWASMUSDT.jsonl

# 需要至少 2 行（2 个快照）
```

如果没有文件或只有 1 行，请**等待更长时间**。

### 问题 2: 有 PnL Summary 但没有平均价格卡片
**原因：** Token 余额没有变化（`baseChange = 0`）

**解决方案：**
```bash
# 查看最近 2 个快照的 ZKWASM 余额
tail -2 .snapshots_gate_ZKWASMUSDT.jsonl | \
  python3 -c "
import json
import sys
for line in sys.stdin:
    data = json.loads(line)
    for asset in data['assets']:
        if asset['asset'] == 'ZKWASM':  # 或你的 token 名称
            print(f'{data[\"datetime\"]}: {asset[\"total\"]:.8f}')
"
```

如果两个快照的余额**完全相同**，说明没有交易发生。需要：
- 等待交易执行
- 或手动下单测试
- 或检查机器人配置

### 问题 3: 显示旧数据
**原因：** 浏览器缓存

**解决方案：**
- Mac: `Cmd + Shift + R`
- Windows/Linux: `Ctrl + Shift + R`

## 🔍 诊断工具

### 运行诊断脚本
```bash
./check_frontend.sh
```

这会检查：
- ✅ 后端是否运行
- ✅ 是否有 PnL 数据
- ✅ 每个交易所的状态
- ✅ 前端构建状态

### 手动检查 API
```bash
# 查看原始 API 数据
curl -s http://localhost:3000/api/report | python3 -m json.tool | grep -A 10 "pnl_summary"

# 查看快照统计
curl -s http://localhost:3000/api/report | \
  python3 -c "
import json
import sys
data = json.load(sys.stdin)
for ex in data['exchanges']:
    pnl = ex.get('pnl_summary')
    if pnl:
        base = pnl['base_asset_summary']
        quote = pnl['quote_asset_summary']
        if base['absolute_change'] != 0:
            avg = abs(quote['absolute_change'] / base['absolute_change'])
            print(f'{ex[\"name\"]}: Avg Price = {avg:.6f}')
        else:
            print(f'{ex[\"name\"]}: No trades yet (baseChange = 0)')
    else:
        print(f'{ex[\"name\"]}: No PnL data (need more snapshots)')
"
```

## 📋 时间线示例

```
时间    | 操作                              | 结果
--------|-----------------------------------|----------------------------------
14:00   | 启动 report 服务                  | 开始收集数据
14:00   | 快照 #1 保存                      | ❌ 还没有 PnL（需要 2 个快照）
14:01   | 机器人执行了一些交易              | 余额发生变化
14:01   | 快照 #2 保存                      | ✅ PnL Summary 出现！
14:01   | 刷新浏览器                        | ✅ 看到平均价格卡片！
14:02   | 快照 #3 保存                      | 更准确的数据
```

## 💡 快速测试

如果你想立即看到效果，可以：

1. **手动制造余额变化：**
   - 在交易所手动下一个小单
   - 等待成交
   - 等待下一个快照（最多 30 秒）

2. **或者使用测试数据：**
   ```bash
   # 运行测试脚本
   ./test_snapshot_generation.sh
   ```

## 🎯 完整流程

```bash
# 1. 确保有最新代码
cd /Users/xingao/mmexecutor
git pull

# 2. 重新编译
cargo build --release

# 3. 启动后端
./target/release/mexc-grid-trader report

# 4. 新终端启动前端
cd frontend
npm start

# 5. 等待 1-2 分钟

# 6. 检查状态
./check_frontend.sh

# 7. 浏览器访问 http://localhost:3001
```

## 📞 仍然有问题？

### 日志检查
```bash
# 查看后端日志
# 应该每 30 秒看到：
# ✅ Report data updated at 14:30:00 UTC
```

### 快照文件检查
```bash
# 查看最新快照
tail -1 .snapshots_gate_ZKWASMUSDT.jsonl | python3 -m json.tool

# 检查文件大小（应该每 30 秒增长）
watch -n 5 'ls -lh .snapshots_*'
```

### 前端检查
```bash
# 确保前端有新代码
grep "Average Trading Price" frontend/build/static/js/main.*.js
# 应该返回：Average Trading Price

# 如果没有，重新构建：
cd frontend
npm run build
```

## 📚 更多信息

- **详细功能说明**: [AVERAGE_PRICE_FEATURE.md](AVERAGE_PRICE_FEATURE.md)
- **快照生成说明**: [REPORT_SNAPSHOT_FEATURE.md](REPORT_SNAPSHOT_FEATURE.md)
- **完整文档**: [REPORT_DASHBOARD.md](REPORT_DASHBOARD.md)

## ✨ 总结

只需 3 步：
1. ✅ 启动 `report` 服务
2. ✅ 等待 1-2 分钟（让它收集数据）
3. ✅ 刷新浏览器查看

就这么简单！🎉
