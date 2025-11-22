use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::Path;

/// 调整类型：充值或提现
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AdjustmentType {
    Deposit,   // 充值
    Withdrawal, // 提现
}

/// 资金调整记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adjustment {
    pub timestamp: i64,
    pub datetime: String,
    pub exchange: String,
    pub asset: String,
    pub amount: f64,
    pub adjustment_type: AdjustmentType,
    pub note: Option<String>,
}

impl Adjustment {
    pub fn new(
        exchange: &str,
        asset: &str,
        amount: f64,
        adjustment_type: AdjustmentType,
        note: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            timestamp: now.timestamp(),
            datetime: now.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            exchange: exchange.to_string(),
            asset: asset.to_string(),
            amount,
            adjustment_type,
            note,
        }
    }
}

/// 调整记录管理器
pub struct AdjustmentHistory {
    file_path: String,
}

impl AdjustmentHistory {
    pub fn new(exchange: &str, symbol: &str) -> Self {
        Self {
            file_path: format!(".adjustments_{}_{}.jsonl", exchange, symbol),
        }
    }

    /// 添加一条调整记录
    pub fn add_adjustment(&self, adjustment: &Adjustment) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .context(format!("Failed to open adjustment file: {}", self.file_path))?;

        let json = serde_json::to_string(adjustment)
            .context("Failed to serialize adjustment")?;

        writeln!(file, "{}", json)
            .context("Failed to write adjustment to file")?;

        Ok(())
    }

    /// 读取所有调整记录
    pub fn load_all(&self) -> Result<Vec<Adjustment>> {
        if !Path::new(&self.file_path).exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.file_path)
            .context(format!("Failed to read adjustment file: {}", self.file_path))?;

        let mut adjustments = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Adjustment>(line) {
                Ok(adjustment) => adjustments.push(adjustment),
                Err(e) => {
                    eprintln!("Warning: Failed to parse line {} in {}: {}",
                             line_num + 1, self.file_path, e);
                }
            }
        }

        Ok(adjustments)
    }

    /// 获取时间段内的调整记录
    pub fn load_between(&self, start_time: i64, end_time: i64) -> Result<Vec<Adjustment>> {
        let all = self.load_all()?;
        Ok(all.into_iter()
            .filter(|adj| adj.timestamp >= start_time && adj.timestamp <= end_time)
            .collect())
    }

    /// 计算时间段内特定资产的净调整
    pub fn calculate_net_adjustment(
        &self,
        asset: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<f64> {
        let adjustments = self.load_between(start_time, end_time)?;

        let mut net = 0.0;
        for adj in adjustments {
            if adj.asset == asset {
                match adj.adjustment_type {
                    AdjustmentType::Deposit => net += adj.amount,
                    AdjustmentType::Withdrawal => net -= adj.amount,
                }
            }
        }

        Ok(net)
    }

    /// 打印所有调整记录
    pub fn print_all(&self) -> Result<()> {
        let adjustments = self.load_all()?;

        if adjustments.is_empty() {
            println!("No adjustments recorded.");
            return Ok(());
        }

        println!("\n╔════════════════════════════════════════════════════════╗");
        println!("║              ADJUSTMENT HISTORY                        ║");
        println!("╚════════════════════════════════════════════════════════╝\n");

        for adj in adjustments {
            let type_icon = match adj.adjustment_type {
                AdjustmentType::Deposit => "📥",
                AdjustmentType::Withdrawal => "📤",
            };
            let type_str = match adj.adjustment_type {
                AdjustmentType::Deposit => "Deposit",
                AdjustmentType::Withdrawal => "Withdrawal",
            };

            println!("{} {} - {} {}", type_icon, adj.datetime, type_str, adj.exchange);
            println!("   Amount: {:+.8} {}", adj.amount, adj.asset);
            if let Some(note) = &adj.note {
                println!("   Note: {}", note);
            }
            println!();
        }

        Ok(())
    }
}
