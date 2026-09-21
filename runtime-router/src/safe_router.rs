use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    context::{snapshot::ContextSnapshot, workload::WorkloadType},
    errors::ContractResult,
    runtime::route::{RuntimeRoute, RuntimeRouteSource},
    telemetry::snapshot::ComputerSnapshot,
    traits::runtime_router::IRuntimeRouter,
};

/// 安全 Runtime 路由器。
///
/// 决策规则（按优先级）：
/// 1. **未知 workload → Native**（不允许为 "AI 化" 强行改变）。
/// 2. **低置信度 → Native**（`context.confidence < min_confidence`）。
/// 3. **AI Inference 且 NPU 可用** → Npu。
/// 4. **AI Inference 且 NPU 不可用但 GPU 可用** → Gpu。
/// 5. **Rendering / VideoEditing 且 GPU 可用** → Gpu。
/// 6. **Compilation 且 CPU 空闲** → Cpu。
/// 7. **其他情况 → Native**。
///
/// 特点：
/// - 默认不改变应用路径（除非明确有收益）。
/// - 每条决策都有 `reason` 与 `confidence`。
/// - 路由失败不影响 Core（router 层无副作用）。
pub struct SafeRuntimeRouter {
    min_confidence: f32,
    next_route: AtomicU64,
}

impl SafeRuntimeRouter {
    pub fn new() -> Self {
        Self {
            min_confidence: 0.5,
            next_route: AtomicU64::new(1),
        }
    }

    pub fn with_min_confidence(mut self, c: f32) -> Self {
        self.min_confidence = c.clamp(0.0, 1.0);
        self
    }

    pub fn min_confidence(&self) -> f32 {
        self.min_confidence
    }

    fn has_npu(&self, snapshot: &ComputerSnapshot) -> bool {
        snapshot.npu.available
    }

    fn has_gpu(&self, snapshot: &ComputerSnapshot) -> bool {
        snapshot.gpu.name.is_some() || snapshot.gpu.usage_percent.is_some()
    }

    fn next_id(&self) -> u64 {
        self.next_route.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for SafeRuntimeRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl IRuntimeRouter for SafeRuntimeRouter {
    fn decide(
        &self,
        snapshot: &ComputerSnapshot,
        context: &ContextSnapshot,
    ) -> ContractResult<RuntimeRoute> {
        // 1. Unknown workload → Native
        if context.workload_type == WorkloadType::Unknown {
            return Ok(with_metadata(
                self.next_id(),
                RuntimeRoute::native(context.workload_type, "unknown workload: keep native path"),
            ));
        }

        // 2. 低置信度 → Native
        if context.confidence < self.min_confidence {
            return Ok(with_metadata(
                self.next_id(),
                RuntimeRoute::native(
                    context.workload_type,
                    format!(
                        "low confidence {:.3} < {:.3}: keep native path",
                        context.confidence, self.min_confidence
                    ),
                ),
            ));
        }

        let has_npu = self.has_npu(snapshot);
        let has_gpu = self.has_gpu(snapshot);

        let route = match context.workload_type {
            WorkloadType::AiInference => {
                if has_npu {
                    RuntimeRoute::routed(
                        context.workload_type,
                        RuntimeRouteSource::Npu,
                        format!(
                            "AI inference with NPU available (npu_usage={:?}%)",
                            snapshot.npu.usage_percent
                        ),
                        context.confidence,
                    )
                } else if has_gpu {
                    RuntimeRoute::routed(
                        context.workload_type,
                        RuntimeRouteSource::Gpu,
                        "AI inference with GPU available (NPU unavailable)",
                        context.confidence * 0.8,
                    )
                } else {
                    RuntimeRoute::native(
                        context.workload_type,
                        "AI inference but no NPU/GPU available",
                    )
                }
            },
            WorkloadType::Rendering | WorkloadType::VideoEditing => {
                if has_gpu {
                    RuntimeRoute::routed(
                        context.workload_type,
                        RuntimeRouteSource::Gpu,
                        format!("{:?} with GPU available", context.workload_type),
                        context.confidence,
                    )
                } else {
                    RuntimeRoute::native(context.workload_type, "GPU unavailable: keep native path")
                }
            },
            WorkloadType::Compilation => {
                let cpu_busy = snapshot.cpu.usage_percent > 70.0;
                if !cpu_busy {
                    RuntimeRoute::routed(
                        context.workload_type,
                        RuntimeRouteSource::Cpu,
                        format!(
                            "compilation with CPU headroom (cpu={:.1}%)",
                            snapshot.cpu.usage_percent
                        ),
                        context.confidence,
                    )
                } else {
                    RuntimeRoute::native(context.workload_type, "CPU busy: keep native path")
                }
            },
            _ => RuntimeRoute::native(
                context.workload_type,
                format!("no explicit route for {:?}", context.workload_type),
            ),
        };

        Ok(with_metadata(self.next_id(), route))
    }

    fn name(&self) -> &'static str {
        "safe_runtime_router"
    }
}

fn with_metadata(id: u64, mut route: RuntimeRoute) -> RuntimeRoute {
    route.route_id = format!("route-{}-{}", now_unix_secs(), id);
    route.decided_at_unix_secs = now_unix_secs();
    route
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
