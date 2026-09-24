use serde::{Deserialize, Serialize};

use crate::application::id::ApplicationId;
use crate::context::workload::WorkloadType;
use crate::policy::resource_policy::PowerMode;

use super::record::HistoricalRecord;

type ApplicationWorkloadCounts = (ApplicationId, u8, Vec<(WorkloadType, usize)>);

/// 从历史记录推导出来的轻量级上下文。
///
/// 供 AI Provider 消费。不包含原始时间序列，只包含聚合结论。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemoryContext {
    pub recent_workloads: Vec<WorkloadType>,
    pub dominant_workload: Option<WorkloadType>,
    pub preferred_power_mode: Option<PowerMode>,
    pub sample_count: usize,
    /// Opt-in aggregate by UTC hour; contains no application names or raw events.
    #[serde(default)]
    pub hourly_patterns: Vec<HourlyWorkloadPattern>,
    /// Opt-in aggregate of explicit application task identifiers.
    #[serde(default)]
    pub application_patterns: Vec<ApplicationHourlyPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationHourlyPattern {
    pub application_id: ApplicationId,
    pub hour_utc: u8,
    pub dominant_workload: WorkloadType,
    pub sample_count: usize,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HourlyWorkloadPattern {
    pub hour_utc: u8,
    pub dominant_workload: WorkloadType,
    pub sample_count: usize,
    pub confidence: f32,
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

        let mut per_hour: [Vec<(WorkloadType, usize)>; 24] = std::array::from_fn(|_| Vec::new());
        for record in records {
            if record.workload_type == WorkloadType::Unknown {
                continue;
            }
            let hour = ((record.timestamp_unix_ms / 3_600_000) % 24) as usize;
            if let Some((_, count)) = per_hour[hour]
                .iter_mut()
                .find(|(workload, _)| *workload == record.workload_type)
            {
                *count += 1;
            } else {
                per_hour[hour].push((record.workload_type, 1));
            }
        }
        let hourly_patterns = per_hour
            .iter()
            .enumerate()
            .filter_map(|(hour, counts)| {
                let total: usize = counts.iter().map(|(_, count)| count).sum();
                if total < 3 {
                    return None;
                }
                let (dominant_workload, dominant_count) =
                    counts.iter().max_by_key(|(_, count)| *count)?;
                Some(HourlyWorkloadPattern {
                    hour_utc: hour as u8,
                    dominant_workload: *dominant_workload,
                    sample_count: total,
                    confidence: *dominant_count as f32 / total as f32,
                })
            })
            .collect();

        let mut per_app: Vec<ApplicationWorkloadCounts> = Vec::new();
        for record in records {
            let Some(app) = &record.application_id else {
                continue;
            };
            if record.workload_type == WorkloadType::Unknown {
                continue;
            }
            let hour = ((record.timestamp_unix_ms / 3_600_000) % 24) as u8;
            let index = match per_app
                .iter()
                .position(|(id, h, _)| id == app && *h == hour)
            {
                Some(index) => index,
                None => {
                    per_app.push((app.clone(), hour, Vec::new()));
                    per_app.len() - 1
                },
            };
            let counts = &mut per_app[index].2;
            if let Some((_, count)) = counts.iter_mut().find(|(w, _)| *w == record.workload_type) {
                *count += 1;
            } else {
                counts.push((record.workload_type, 1));
            }
        }
        let application_patterns = per_app
            .into_iter()
            .filter_map(|(application_id, hour_utc, counts)| {
                let sample_count: usize = counts.iter().map(|(_, count)| count).sum();
                if sample_count < 3 {
                    return None;
                }
                let (dominant_workload, dominant_count) =
                    counts.iter().max_by_key(|(_, count)| *count)?;
                Some(ApplicationHourlyPattern {
                    application_id,
                    hour_utc,
                    dominant_workload: *dominant_workload,
                    sample_count,
                    confidence: *dominant_count as f32 / sample_count as f32,
                })
            })
            .collect();

        Self {
            recent_workloads: recent,
            dominant_workload: dominant,
            preferred_power_mode: preferred_power,
            sample_count: records.len(),
            hourly_patterns,
            application_patterns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        context::workload::InteractionLevel, execution::status::ExecutionOverallStatus,
        policy::priority::PolicySource,
    };

    fn record(day: u64, workload_type: WorkloadType) -> HistoricalRecord {
        HistoricalRecord {
            application_id: None,
            record_id: format!("day-{}", day),
            timestamp_unix_ms: (day * 24 + 14) * 3_600_000,
            workload_type,
            interaction_level: InteractionLevel::Interactive,
            policy_id: "policy".into(),
            policy_source: PolicySource::RuleBased,
            overall_status: ExecutionOverallStatus::AllSucceeded,
            cpu_usage_percent: 20.0,
            gpu_usage_percent: None,
            npu_usage_percent: None,
            memory_usage_percent: 30.0,
            power_mode: PowerMode::Balanced,
        }
    }

    #[test]
    fn aggregates_utc_hour_without_raw_application_data() {
        let records = vec![
            record(0, WorkloadType::Development),
            record(1, WorkloadType::Development),
            record(2, WorkloadType::Development),
            record(3, WorkloadType::Gaming),
        ];
        let context = MemoryContext::from_records(&records);
        assert_eq!(context.hourly_patterns.len(), 1);
        let pattern = &context.hourly_patterns[0];
        assert_eq!(pattern.hour_utc, 14);
        assert_eq!(pattern.dominant_workload, WorkloadType::Development);
        assert_eq!(pattern.sample_count, 4);
        assert!((pattern.confidence - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn explicit_application_pattern_is_separate_and_legacy_record_stays_readable() {
        let app = ApplicationId::new("app.editor");
        let mut records = vec![
            record(0, WorkloadType::Development),
            record(1, WorkloadType::Development),
            record(2, WorkloadType::Development),
        ];
        for entry in &mut records {
            entry.application_id = Some(app.clone());
        }
        let other = record(3, WorkloadType::Gaming);
        records.push(other);
        let context = MemoryContext::from_records(&records);
        assert_eq!(context.application_patterns.len(), 1);
        assert_eq!(context.application_patterns[0].application_id, app);
        assert_eq!(context.application_patterns[0].sample_count, 3);
        let mut legacy = serde_json::to_value(&records[0]).unwrap();
        legacy.as_object_mut().unwrap().remove("application_id");
        let parsed: HistoricalRecord = serde_json::from_value(legacy).unwrap();
        assert_eq!(parsed.application_id, None);
    }
}
