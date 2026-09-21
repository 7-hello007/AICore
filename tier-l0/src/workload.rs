use std::sync::Arc;

use aicore_contracts::{
    errors::ContractResult,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        task_classifier::{ClassificationInput, ITaskClassifier},
        workload_source::{IWorkloadSource, WorkloadObservation},
    },
};
use aicore_core::context::RuleBasedClassifier;

/// L0 workload source：基于规则分类器从系统状态推断 workload。
///
/// 特点：
/// - 零改动软件：不要求应用声明任何信息。
/// - 确定性：规则优先，无 AI 推断。
/// - 低开销：不做子进程调用（`ComputerSnapshot` 已包含全部输入）。
pub struct L0WorkloadSource {
    classifier: Arc<dyn ITaskClassifier>,
}

impl L0WorkloadSource {
    pub fn new() -> Self {
        Self {
            classifier: Arc::new(RuleBasedClassifier::new()),
        }
    }

    pub fn with_classifier(classifier: Arc<dyn ITaskClassifier>) -> Self {
        Self { classifier }
    }
}

impl Default for L0WorkloadSource {
    fn default() -> Self {
        Self::new()
    }
}

impl IWorkloadSource for L0WorkloadSource {
    fn observe(&self, snapshot: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        let input = build_input(snapshot);
        let workload = self.classifier.classify(&input);
        let confidence = self.classifier.confidence(&input).clamp(0.0, 1.0);

        let mut evidence = Vec::new();
        if let Some(fg) = &input.foreground_app {
            evidence.push(format!("foreground_app={}", fg));
        }
        if let Some(p) = &input.foreground_process {
            evidence.push(format!("foreground_process={}", p));
        }
        evidence.push(format!(
            "active_processes={}",
            input.active_process_names.len()
        ));
        evidence.push(format!("rule_match={:?}", workload));

        Ok(WorkloadObservation {
            workload_type: workload,
            confidence,
            evidence,
        })
    }

    fn source_name(&self) -> &'static str {
        "l0.workload_source"
    }

    fn priority(&self) -> u32 {
        // L0 是最低优先级：L1/L2 的显式声明会覆盖 L0 推断。
        100
    }
}

fn build_input(snapshot: &ComputerSnapshot) -> ClassificationInput {
    let foreground_app = snapshot
        .applications
        .iter()
        .find(|a| a.is_foreground)
        .or_else(|| snapshot.applications.first())
        .cloned();

    let foreground_process = snapshot
        .top_processes
        .iter()
        .find(|p| p.is_foreground)
        .cloned();

    ClassificationInput {
        cpu_usage_percent: snapshot.cpu.usage_percent,
        gpu_usage_percent: snapshot.gpu.usage_percent,
        npu_usage_percent: snapshot.npu.usage_percent,
        memory_usage_percent: snapshot.memory.usage_percent,
        foreground_app: foreground_app.as_ref().map(|a| a.name.clone()),
        foreground_process: foreground_process.as_ref().map(|p| p.name.clone()),
        foreground_window: foreground_app.as_ref().and_then(|a| a.window_title.clone()),
        active_process_names: snapshot
            .top_processes
            .iter()
            .map(|p| p.name.clone())
            .collect(),
    }
}
