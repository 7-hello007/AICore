use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;
use crate::policy::resource_policy::PowerMode;

use super::record::HistoricalRecord;

/// 从历史记录推导出来的轻量级上下文。
///
/// 供 AI Provider 消费。不包含原始时间序列，只包含聚合结论。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemoryContext {
    pub recent_workloads: Vec<WorkloadType>,
    pub dominant_workload: Option<WorkloadType>,
    pub preferred_power_mode: Option<PowerMode>,
    pub sample_count: usize,
}

impl MemoryContext {
    /// 从历史记录构建。空输入返回默认值。
    pub fn from_records(records: &[HistoricalRecord]) -> Self {
        if records.is_empty() {
            return Self::default();
        }

        // 主导 workload
        let mut counts: Vec<(WorkloadType, usize)> = Vec::new();
        for r in records {
            if let Some(entry) = counts.iter_mut().find(|(w, _)| *w == r.workload_type) {
                entry.1 += 1;
            } else {
                counts.push((r.workload_type, 1));
            }
        }
        let dominant = counts.iter().max_by_key(|(_, c)| *c).map(|(w, _)| *w);

        // 最近序列（新 → 旧）
        let recent: Vec<WorkloadType> = records
            .iter()
            .rev()
            .take(8)
            .map(|r| r.workload_type)
            .collect();

        // 首选电源模式
        let mut power_counts: Vec<(PowerMode, usize)> = Vec::new();
        for r in records {
            if let Some(entry) = power_counts.iter_mut().find(|(m, _)| *m == r.power_mode) {
                entry.1 += 1;
            } else {
                power_counts.push((r.power_mode, 1));
            }
        }
        let preferred_power = power_counts.iter().max_by_key(|(_, c)| *c).map(|(m, _)| *m);

        Self {
            recent_workloads: recent,
            dominant_workload: dominant,
            preferred_power_mode: preferred_power,
            sample_count: records.len(),
        }
    }
}
