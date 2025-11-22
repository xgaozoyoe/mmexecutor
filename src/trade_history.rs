use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use crate::exchange::Trade;

/// 交易历史管理
pub struct TradeHistory {
    file_path: String,
}

impl TradeHistory {
    pub fn new(exchange: &str, symbol: &str) -> Self {
        Self {
            file_path: format!(".trades_{}_{}.jsonl", exchange, symbol),
        }
    }

    /// 添加交易记录（批量）
    pub fn add_trades(&self, trades: &[Trade]) -> Result<()> {
        if trades.is_empty() {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;

        for trade in trades {
            let json = serde_json::to_string(trade)?;
            writeln!(file, "{}", json)?;
        }

        Ok(())
    }

    /// 加载所有交易记录
    pub fn load_all(&self) -> Result<Vec<Trade>> {
        if !Path::new(&self.file_path).exists() {
            return Ok(Vec::new());
        }

        let file = std::fs::File::open(&self.file_path)?;
        let reader = BufReader::new(file);
        let mut trades = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let trade: Trade = serde_json::from_str(&line)?;
            trades.push(trade);
        }

        Ok(trades)
    }

    /// 获取最后一笔交易的时间戳
    pub fn get_last_trade_time(&self) -> Result<Option<i64>> {
        let trades = self.load_all()?;
        Ok(trades.iter().map(|t| t.time).max())
    }

    /// 获取指定时间范围内的交易
    pub fn get_trades_in_range(&self, start_time: i64, end_time: i64) -> Result<Vec<Trade>> {
        let all_trades = self.load_all()?;
        Ok(all_trades
            .into_iter()
            .filter(|t| t.time >= start_time && t.time <= end_time)
            .collect())
    }

    /// 检查某笔交易是否已存在（通过 trade ID）
    pub fn trade_exists(&self, trade_id: &str) -> Result<bool> {
        let trades = self.load_all()?;
        Ok(trades.iter().any(|t| t.id == trade_id))
    }

    /// 去重并添加交易（只添加不存在的交易）
    /// 使用 (id, order_id) 组合作为唯一键，因为某些交易所（如Gate.io）
    /// 可能对同一笔市场成交返回多条记录（买卖双方）
    pub fn add_trades_deduplicated(&self, new_trades: &[Trade]) -> Result<usize> {
        let existing_trades = self.load_all()?;
        let existing_keys: std::collections::HashSet<_> =
            existing_trades.iter()
                .map(|t| (t.id.as_str(), t.order_id.as_str()))
                .collect();

        let trades_to_add: Vec<_> = new_trades
            .iter()
            .filter(|t| !existing_keys.contains(&(t.id.as_str(), t.order_id.as_str())))
            .collect();

        let count = trades_to_add.len();

        if count > 0 {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)?;

            for trade in trades_to_add {
                let json = serde_json::to_string(trade)?;
                writeln!(file, "{}", json)?;
            }
        }

        Ok(count)
    }
}
