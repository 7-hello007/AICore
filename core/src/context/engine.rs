use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    context::{snapshot::ContextSnapshot, workload::InteractionLevel},
    traits::{
        task_classifier::{ClassificationInput, ITaskClassifier},
        workload_source::IWorkloadSource,
    },
};

use crate::domain::computer_state::ComputerState;

pub struct ContextEngine {
    classifier: Arc<dyn ITaskClassifier>,
    workload_source: Option<Arc<dyn IWorkloadSource>>,
    max_active_apps: usize,
}

impl ContextEngine {
    pub fn new(classifier: Arc<dyn ITaskClassifier>) -> Self {
        Self {
            classifier,
            workload_source: None,
            max_active_apps: 16,
        }
    }

    pub fn with_workload_source(mut self, source: Arc<dyn IWorkloadSource>) -> Self {
        self.workload_source = Some(source);
        self
    }

    pub fn with_max_active_apps(mut self, max: usize) -> Self {
        self.max_active_apps = max.max(1);
        self
    }

    pub fn classifier_name(&self) -> &'static str {
        self.classifier.name()
    }

    pub fn build(&self, state: &ComputerState) -> ContextSnapshot {
        self.build_with_workload_source(state, None)
    }

    pub fn build_with_workload_source(
        &self,
        state: &ComputerState,
        source_override: Option<Arc<dyn IWorkloadSource>>,
    ) -> ContextSnapshot {
        let snapshot = &state.snapshot;
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

        let mut active_apps = Vec::with_capacity(self.max_active_apps);
        let mut seen = HashSet::new();
        for a in &snapshot.applications {
            if seen.insert(a.name.clone()) {
                active_apps.push(a.name.clone());
            }
            if active_apps.len() >= self.max_active_apps {
                break;
            }
        }
        if active_apps.is_empty() {
            for p in &snapshot.top_processes {
                if seen.insert(p.name.clone()) {
                    active_apps.push(p.name.clone());
                }
                if active_apps.len() >= self.max_active_apps {
                    break;
                }
            }
        }

        let input = ClassificationInput {
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
        };

        let source = source_override.or_else(|| self.workload_source.clone());
        let (workload_type, confidence) = if let Some(source) = source {
            match source.observe(snapshot) {
                Ok(obs) => (obs.workload_type, obs.confidence.clamp(0.0, 1.0)),
                Err(err) => {
                    tracing::debug!(component="context_engine", event="workload_source_failed", source=source.source_name(), error=%err);
                    (
                        self.classifier.classify(&input),
                        self.classifier.confidence(&input).clamp(0.0, 1.0),
                    )
                },
            }
        } else {
            (
                self.classifier.classify(&input),
                self.classifier.confidence(&input).clamp(0.0, 1.0),
            )
        };

        let interaction_level = determine_interaction_level(&input, workload_type);
        ContextSnapshot {
            timestamp_unix_ms: Some(now_unix_ms()),
            foreground_app: foreground_app.map(|a| a.name),
            foreground_window_title: input.foreground_window,
            foreground_process_name: foreground_process.map(|p| p.name),
            active_apps,
            workload_type,
            interaction_level,
            confidence,
        }
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn determine_interaction_level(
    input: &ClassificationInput,
    workload: aicore_contracts::context::workload::WorkloadType,
) -> InteractionLevel {
    use aicore_contracts::context::workload::WorkloadType;
    if input.foreground_app.is_none() && input.foreground_process.is_none() {
        return InteractionLevel::Idle;
    }
    match workload {
        WorkloadType::VideoEditing
        | WorkloadType::Rendering
        | WorkloadType::Compilation
        | WorkloadType::FileProcessing
        | WorkloadType::BackgroundTask => InteractionLevel::Passive,
        WorkloadType::Unknown => {
            if input.cpu_usage_percent < 5.0 && input.memory_usage_percent < 30.0 {
                InteractionLevel::Idle
            } else {
                InteractionLevel::Passive
            }
        },
        _ => InteractionLevel::Interactive,
    }
}
