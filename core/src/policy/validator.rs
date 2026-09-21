use aicore_contracts::{
    policy::{priority::PolicyPriority, resource_policy::ResourcePolicy},
    traits::policy_validator::{IPolicyValidator, ValidationIssue, ValidationResult},
};

/// 默认策略验证器。
///
/// 拒绝规则：
/// - ERR_POLICY_INVALID：policy_id 为空、字段超出合法范围。
/// - ERR_POLICY_CONFLICT：内部不一致（子资源优先级超过整体、NPU preferred 与 Background 冲突）。
pub struct DefaultPolicyValidator;

impl DefaultPolicyValidator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultPolicyValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl IPolicyValidator for DefaultPolicyValidator {
    fn validate(&self, policy: &ResourcePolicy) -> ValidationResult {
        let mut issues = Vec::new();

        if policy.policy_id.trim().is_empty() {
            issues.push(issue("ERR_POLICY_INVALID", "policy_id is empty"));
        }

        // 子资源优先级不得超过整体优先级
        let overall = policy.priority;
        for (name, p) in [
            ("cpu", policy.cpu.priority),
            ("gpu", policy.gpu.priority),
            ("npu", policy.npu.priority),
            ("memory", policy.memory.priority),
        ] {
            if p > overall {
                issues.push(issue(
                    "ERR_POLICY_CONFLICT",
                    &format!(
                        "{} priority ({:?}) exceeds overall priority ({:?})",
                        name, p, overall
                    ),
                ));
            }
        }

        if let Some(max) = policy.background.max_cpu_percent {
            if !(0.0..=100.0).contains(&max) {
                issues.push(issue(
                    "ERR_POLICY_INVALID",
                    "background.max_cpu_percent out of range [0, 100]",
                ));
            }
        }

        if let Some(max) = policy.memory.max_usage_percent {
            if !(0.0..=100.0).contains(&max) {
                issues.push(issue(
                    "ERR_POLICY_INVALID",
                    "memory.max_usage_percent out of range [0, 100]",
                ));
            }
        }

        if policy.npu.preferred && policy.npu.priority == PolicyPriority::Background {
            issues.push(issue(
                "ERR_POLICY_CONFLICT",
                "npu.preferred=true but npu.priority=Background",
            ));
        }

        if let Some(cores) = &policy.cpu.affinity_cores {
            if cores.is_empty() {
                issues.push(issue("ERR_POLICY_INVALID", "cpu.affinity_cores is empty"));
            }
        }

        ValidationResult {
            accepted: issues.is_empty(),
            issues,
        }
    }

    fn name(&self) -> &'static str {
        "default_policy_validator"
    }
}

fn issue(code: &str, message: &str) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        message: message.to_string(),
    }
}
