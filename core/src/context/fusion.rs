use std::collections::HashMap;
use std::sync::Arc;

use aicore_contracts::{
    context::workload::WorkloadType,
    errors::ContractResult,
    telemetry::ComputerSnapshot,
    traits::workload_source::{IWorkloadSource, SourceKind, WorkloadObservation},
};

pub struct WorkloadFusion {
    sources: Vec<Arc<dyn IWorkloadSource>>,
    high_confidence_threshold: f32,
    ml_high_confidence_threshold: f32,
    agreement_boost_threshold: usize,
    agreement_boost: f32,
    min_return_confidence: f32,
}

impl WorkloadFusion {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            high_confidence_threshold: 0.9,
            ml_high_confidence_threshold: 0.95,
            agreement_boost_threshold: 2,
            agreement_boost: 0.1,
            min_return_confidence: 0.5,
        }
    }
    pub fn add_source(&mut self, source: Arc<dyn IWorkloadSource>) {
        self.sources.push(source);
        self.sources
            .sort_by_key(|s| std::cmp::Reverse(s.priority()));
    }
    pub fn with_source(mut self, source: Arc<dyn IWorkloadSource>) -> Self {
        self.add_source(source);
        self
    }
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
    pub fn min_return_confidence(&self) -> f32 {
        self.min_return_confidence
    }
    pub fn with_min_return_confidence(mut self, c: f32) -> Self {
        self.min_return_confidence = c.clamp(0.0, 1.0);
        self
    }

    fn collect(
        &self,
        snapshot: &ComputerSnapshot,
    ) -> Vec<(Arc<dyn IWorkloadSource>, WorkloadObservation)> {
        let mut out = Vec::new();
        for src in &self.sources {
            match src.observe(snapshot) {
                Ok(obs) if obs.workload_type != WorkloadType::Unknown => {
                    out.push((src.clone(), obs))
                },
                Ok(_) => {},
                Err(e) => {
                    tracing::debug!(component="workload_fusion", event="source_error", source=src.source_name(), error=%e, "source returned error, skipping")
                },
            }
        }
        out
    }

    fn try_direct_accept(
        &self,
        observations: &[(Arc<dyn IWorkloadSource>, WorkloadObservation)],
    ) -> Option<WorkloadObservation> {
        for kind_pref in [SourceKind::Rule, SourceKind::Declared, SourceKind::Ml] {
            for (src, obs) in observations {
                if src.kind() != kind_pref {
                    continue;
                }
                let threshold = if src.kind() == SourceKind::Ml {
                    self.ml_high_confidence_threshold
                } else {
                    self.high_confidence_threshold
                };
                if obs.confidence.clamp(0.0, 1.0) >= threshold {
                    return Some(obs.clone());
                }
            }
        }
        None
    }

    fn merge_by_vote(
        &self,
        observations: &[(Arc<dyn IWorkloadSource>, WorkloadObservation)],
    ) -> WorkloadObservation {
        let mut votes: HashMap<WorkloadType, (u32, f32, Vec<String>)> = HashMap::new();
        for (_, obs) in observations {
            let entry = votes
                .entry(obs.workload_type)
                .or_insert((0, 0.0, Vec::new()));
            entry.0 += 1;
            entry.1 = entry.1.max(obs.confidence);
            entry.2.extend(obs.evidence.iter().cloned());
        }
        let mut entries: Vec<_> = votes.into_iter().collect();
        entries.sort_by(|a, b| {
            b.1 .0.cmp(&a.1 .0).then_with(|| {
                b.1 .1
                    .partial_cmp(&a.1 .1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        let (workload, (vote_count, base_conf, mut evidence)) =
            entries.into_iter().next().expect("non-empty observations");
        let mut confidence = base_conf.clamp(0.0, 1.0);
        if vote_count as usize >= self.agreement_boost_threshold {
            confidence = (confidence + self.agreement_boost).min(1.0);
            evidence.push(format!("fusion: {} sources agree", vote_count));
        }
        WorkloadObservation {
            workload_type: workload,
            confidence,
            evidence,
        }
    }
}

impl Default for WorkloadFusion {
    fn default() -> Self {
        Self::new()
    }
}

impl IWorkloadSource for WorkloadFusion {
    fn observe(&self, snapshot: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        let observations = self.collect(snapshot);
        if observations.is_empty() {
            return Ok(WorkloadObservation {
                workload_type: WorkloadType::Unknown,
                confidence: 0.0,
                evidence: vec!["no source produced a result".to_string()],
            });
        }
        if let Some(direct) = self.try_direct_accept(&observations) {
            return Ok(direct);
        }
        let merged = self.merge_by_vote(&observations);
        if merged.confidence < self.min_return_confidence {
            return Ok(WorkloadObservation {
                workload_type: WorkloadType::Unknown,
                confidence: merged.confidence,
                evidence: vec![format!(
                    "fusion: low confidence {:.3} < {:.3}, abstaining",
                    merged.confidence, self.min_return_confidence
                )],
            });
        }
        Ok(merged)
    }
    fn source_name(&self) -> &'static str {
        "workload_fusion"
    }
    fn priority(&self) -> u32 {
        1000
    }
}
