use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::Path;

/// 单个资产的快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSnapshot {
    pub asset: String,
    pub free: f64,
    pub locked: f64,
    pub total: f64,
}

/// 账户快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub timestamp: i64,
    pub datetime: String,
    pub exchange: String,
    pub symbol: String,
    pub assets: Vec<AssetSnapshot>,
    pub mid_price: Option<f64>,  // 当时的中间价
    pub iteration: Option<u64>,   // watch 模式的迭代次数
}

impl AccountSnapshot {
    pub fn new(
        exchange: &str,
        symbol: &str,
        assets: Vec<AssetSnapshot>,
        mid_price: Option<f64>,
        iteration: Option<u64>,
    ) -> Self {
        let now = Utc::now();
        Self {
            timestamp: now.timestamp(),
            datetime: now.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            assets,
            mid_price,
            iteration,
        }
    }

    /// 计算账户总价值（以 quote 资产计价）
    pub fn calculate_total_value(&self, base_asset: &str, quote_asset: &str) -> Option<f64> {
        if let Some(price) = self.mid_price {
            let mut total_value = 0.0;

            for asset in &self.assets {
                if asset.asset == quote_asset {
                    total_value += asset.total;
                } else if asset.asset == base_asset {
                    total_value += asset.total * price;
                }
            }

            Some(total_value)
        } else {
            None
        }
    }

    /// 获取特定资产的余额
    pub fn get_asset_balance(&self, asset: &str) -> Option<&AssetSnapshot> {
        self.assets.iter().find(|a| a.asset == asset)
    }
}

/// 快照历史管理器
pub struct SnapshotHistory {
    file_path: String,
}

impl SnapshotHistory {
    pub fn new(exchange: &str, symbol: &str) -> Self {
        Self {
            file_path: format!(".account_snapshots_{}_{}.jsonl", exchange, symbol),
        }
    }

    /// 追加一个快照到历史记录
    pub fn append_snapshot(&self, snapshot: &AccountSnapshot) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .context(format!("Failed to open snapshot file: {}", self.file_path))?;

        let json = serde_json::to_string(snapshot)
            .context("Failed to serialize snapshot")?;

        writeln!(file, "{}", json)
            .context("Failed to write snapshot to file")?;

        Ok(())
    }

    /// 读取所有快照历史
    pub fn load_all(&self) -> Result<Vec<AccountSnapshot>> {
        if !Path::new(&self.file_path).exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.file_path)
            .context(format!("Failed to read snapshot file: {}", self.file_path))?;

        let mut snapshots = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<AccountSnapshot>(line) {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(e) => {
                    eprintln!("Warning: Failed to parse line {} in {}: {}",
                             line_num + 1, self.file_path, e);
                }
            }
        }

        Ok(snapshots)
    }

    /// 获取最近 N 个快照
    pub fn load_recent(&self, count: usize) -> Result<Vec<AccountSnapshot>> {
        let all = self.load_all()?;
        let start = if all.len() > count {
            all.len() - count
        } else {
            0
        };
        Ok(all[start..].to_vec())
    }

    /// 获取最新的快照
    pub fn load_latest(&self) -> Result<Option<AccountSnapshot>> {
        let snapshots = self.load_all()?;
        Ok(snapshots.last().cloned())
    }
}

/// 盈亏分析器
pub struct PnLAnalyzer;

impl PnLAnalyzer {
    /// 分析两个快照之间的变化
    pub fn analyze_change(
        earlier: &AccountSnapshot,
        later: &AccountSnapshot,
        base_asset: &str,
        quote_asset: &str,
    ) -> PnLReport {
        let mut asset_changes = HashMap::new();

        // 计算资产变化
        for asset in &later.assets {
            let earlier_asset = earlier.get_asset_balance(&asset.asset);
            let earlier_total = earlier_asset.map(|a| a.total).unwrap_or(0.0);
            let change = asset.total - earlier_total;

            asset_changes.insert(asset.asset.clone(), AssetChange {
                asset: asset.asset.clone(),
                earlier_balance: earlier_total,
                later_balance: asset.total,
                change,
            });
        }

        // 计算总价值变化
        let earlier_value = earlier.calculate_total_value(base_asset, quote_asset);
        let later_value = later.calculate_total_value(base_asset, quote_asset);

        let value_change = match (earlier_value, later_value) {
            (Some(e), Some(l)) => {
                let change = l - e;

                Some(ValueChange {
                    earlier_value: e,
                    later_value: l,
                    change,
                    change_percentage: if e != 0.0 { (change / e) * 100.0 } else { 0.0 },
                })
            },
            _ => None,
        };

        PnLReport {
            earlier_snapshot: earlier.clone(),
            later_snapshot: later.clone(),
            asset_changes,
            value_change,
            time_elapsed_seconds: later.timestamp - earlier.timestamp,
        }
    }

    /// 分析一段时间内的整体表现
    pub fn analyze_period(
        snapshots: &[AccountSnapshot],
        base_asset: &str,
        quote_asset: &str,
    ) -> Option<PeriodAnalysis> {
        if snapshots.len() < 2 {
            return None;
        }

        let first = &snapshots[0];
        let last = &snapshots[snapshots.len() - 1];

        let overall_change = Self::analyze_change(first, last, base_asset, quote_asset);

        // 计算每次迭代的盈亏
        let mut iteration_changes = Vec::new();
        for i in 1..snapshots.len() {
            let change = Self::analyze_change(&snapshots[i - 1], &snapshots[i], base_asset, quote_asset);
            iteration_changes.push(change);
        }

        // 统计盈利次数
        let profitable_iterations = iteration_changes
            .iter()
            .filter(|c| {
                if let Some(vc) = &c.value_change {
                    vc.change > 0.0
                } else {
                    false
                }
            })
            .count();

        Some(PeriodAnalysis {
            total_snapshots: snapshots.len(),
            total_iterations: snapshots.len() - 1,
            profitable_iterations,
            overall_change,
            iteration_changes,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AssetChange {
    pub asset: String,
    pub earlier_balance: f64,
    pub later_balance: f64,
    pub change: f64,
}

#[derive(Debug, Clone)]
pub struct ValueChange {
    pub earlier_value: f64,
    pub later_value: f64,
    pub change: f64,
    pub change_percentage: f64,
}

#[derive(Debug, Clone)]
pub struct PnLReport {
    pub earlier_snapshot: AccountSnapshot,
    pub later_snapshot: AccountSnapshot,
    pub asset_changes: HashMap<String, AssetChange>,
    pub value_change: Option<ValueChange>,
    pub time_elapsed_seconds: i64,
}

impl PnLReport {
    pub fn print_summary(&self, base_asset: &str, quote_asset: &str) {
        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║              PROFIT & LOSS SUMMARY                     ║");
        println!("╚════════════════════════════════════════════════════════╝");

        println!("\n📅 Time Period:");
        println!("  From: {}", self.earlier_snapshot.datetime);
        println!("  To:   {}", self.later_snapshot.datetime);
        println!("  Duration: {} seconds ({:.1} minutes)",
                 self.time_elapsed_seconds,
                 self.time_elapsed_seconds as f64 / 60.0);

        if let Some(vc) = &self.value_change {
            println!("\n💰 Total Value Change (in {}):", quote_asset);
            println!("  Before: {:.2}", vc.earlier_value);
            println!("  After:  {:.2}", vc.later_value);
            let icon = if vc.change >= 0.0 { "📈" } else { "📉" };
            println!("  {} Net P&L: {:.2} ({:+.3}%)", icon, vc.change, vc.change_percentage);
        }

        println!("\n🪙 Asset Changes:");
        for asset in [base_asset, quote_asset] {
            if let Some(change) = self.asset_changes.get(asset) {
                let icon = if change.change >= 0.0 { "⬆" } else { "⬇" };
                println!("  {} {:<8} {:>15.8} → {:>15.8} ({:+.8})",
                         icon,
                         change.asset,
                         change.earlier_balance,
                         change.later_balance,
                         change.change);
            }
        }

        println!("════════════════════════════════════════════════════════\n");
    }
}

#[derive(Debug)]
pub struct PeriodAnalysis {
    pub total_snapshots: usize,
    pub total_iterations: usize,
    pub profitable_iterations: usize,
    pub overall_change: PnLReport,
    pub iteration_changes: Vec<PnLReport>,
}

impl PeriodAnalysis {
    pub fn print_detailed_report(&self, _base_asset: &str, quote_asset: &str) {
        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║            DETAILED PERIOD ANALYSIS                    ║");
        println!("╚════════════════════════════════════════════════════════╝");

        println!("\n📊 Overall Statistics:");
        println!("  Total Snapshots: {}", self.total_snapshots);
        println!("  Total Iterations: {}", self.total_iterations);
        println!("  Profitable Iterations: {} ({:.1}%)",
                 self.profitable_iterations,
                 (self.profitable_iterations as f64 / self.total_iterations as f64) * 100.0);

        if let Some(vc) = &self.overall_change.value_change {
            println!("\n💰 Overall Performance:");
            println!("  Initial Value: {:.2} {}", vc.earlier_value, quote_asset);
            println!("  Final Value:   {:.2} {}", vc.later_value, quote_asset);
            let icon = if vc.change >= 0.0 { "📈" } else { "📉" };
            println!("  {} Total P&L:    {:.2} {} ({:+.3}%)",
                     icon, vc.change, quote_asset, vc.change_percentage);

            // 计算平均每次迭代的收益
            if self.total_iterations > 0 {
                let avg_per_iteration = vc.change / self.total_iterations as f64;
                println!("  Average per iteration: {:.2} {}", avg_per_iteration, quote_asset);
            }
        }

        println!("\n📈 Recent Iterations:");
        let recent_count = 5.min(self.iteration_changes.len());
        for (i, change) in self.iteration_changes.iter()
            .rev()
            .take(recent_count)
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .enumerate()
        {
            if let Some(vc) = &change.value_change {
                let icon = if vc.change >= 0.0 { "✅" } else { "❌" };
                let iter_num = self.total_iterations - recent_count + i + 1;
                println!("  {} Iteration #{}: {:+.2} {} ({:+.3}%)",
                         icon, iter_num, vc.change, quote_asset, vc.change_percentage);
            }
        }

        println!("════════════════════════════════════════════════════════\n");
    }
}
