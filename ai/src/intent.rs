use std::sync::{mpsc, Arc};
use std::time::Duration;

use aicore_contracts::{
    ai::intent::{IntentDeployment, IntentProposal, IntentRequest},
    context::snapshot::ContextSnapshot,
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    memory::profile::UserProfile,
    policy::resource_policy::ResourcePolicy,
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        intent_agent::IIntentAgent, policy_engine::IPolicyEngine,
        policy_validator::IPolicyValidator,
    },
};

/// A single bounded worker prevents a slow local/remote agent from blocking
/// the policy caller indefinitely. Timed-out results are discarded.
pub struct BoundedIntentAgent {
    requests: mpsc::SyncSender<(
        IntentRequest,
        mpsc::SyncSender<ContractResult<IntentProposal>>,
    )>,
    timeout: Duration,
    deployment: IntentDeployment,
}

impl BoundedIntentAgent {
    pub fn new(agent: Arc<dyn IIntentAgent>, timeout: Duration) -> ContractResult<Self> {
        if timeout.is_zero() {
            return Err(ContractError::ContractInvalid(
                "intent deadline must be positive".into(),
            ));
        }
        let deployment = agent.deployment();
        let (tx, rx) = mpsc::sync_channel::<(
            IntentRequest,
            mpsc::SyncSender<ContractResult<IntentProposal>>,
        )>(1);
        std::thread::Builder::new()
            .name("aicore-intent-agent".into())
            .spawn(move || {
                while let Ok((request, reply)) = rx.recv() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        agent.propose(&request)
                    }))
                    .unwrap_or_else(|_| {
                        Err(ContractError::AiInferenceFailed(
                            "intent agent panicked".into(),
                        ))
                    });
                    let _ = reply.send(result);
                }
            })
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        Ok(Self {
            requests: tx,
            timeout,
            deployment,
        })
    }
}

impl IIntentAgent for BoundedIntentAgent {
    fn propose(&self, request: &IntentRequest) -> ContractResult<IntentProposal> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.requests
            .try_send((request.clone(), reply))
            .map_err(|error| ContractError::AiTimeout(error.to_string()))?;
        rx.recv_timeout(self.timeout)
            .map_err(|error| ContractError::AiTimeout(error.to_string()))?
    }

    fn deployment(&self) -> IntentDeployment {
        self.deployment
    }

    fn name(&self) -> &'static str {
        "bounded_intent_agent"
    }
}

/// Maps a proposal through the existing policy engine and validator. It never
/// applies the policy; a rejected or stale proposal is not an execution.
pub struct IntentPolicyContext<'a> {
    pub profile: &'a UserProfile,
    pub snapshot: &'a ComputerSnapshot,
    pub context: &'a ContextSnapshot,
    pub now_unix_ms: u64,
}

pub fn validated_intent_policy(
    agent: &dyn IIntentAgent,
    request: &IntentRequest,
    inputs: IntentPolicyContext<'_>,
    engine: &dyn IPolicyEngine,
    validator: &dyn IPolicyValidator,
) -> ContractResult<ResourcePolicy> {
    if request.text.trim().is_empty() || request.text.len() > 4096 {
        return Err(ContractError::ContractInvalid("invalid intent text".into()));
    }
    if agent.deployment() == IntentDeployment::Cloud && !inputs.profile.can_use_cloud() {
        return Err(ContractError::PermissionDenied(
            "cloud intent inference not consented".into(),
        ));
    }
    let proposal = agent.propose(request)?;
    validate_intent_proposal(&proposal, inputs.now_unix_ms)?;
    let mut proposed_context = inputs.context.clone();
    proposed_context.workload_type = proposal.workload_type;
    let policy = engine.generate(inputs.snapshot, &proposed_context);
    let validation = validator.validate(&policy);
    if !validation.accepted {
        return Err(ContractError::ContractInvalid(format!(
            "intent-derived policy rejected: {:?}",
            validation.issues
        )));
    }
    Ok(policy)
}

/// Shared gate for local intent API and policy conversion. Semantic proposals
/// are never accepted as executable commands.
pub fn validate_intent_proposal(proposal: &IntentProposal, now_unix_ms: u64) -> ContractResult<()> {
    if proposal.workload_type == WorkloadType::Unknown
        || !proposal.confidence.is_finite()
        || proposal.confidence < 0.6
        || proposal.confidence > 1.0
        || proposal.expires_at_unix_ms <= now_unix_ms
    {
        return Err(ContractError::ContractInvalid(
            "unusable intent proposal".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::{
        application::id::ApplicationId, policy::priority::PolicyPriority,
        traits::policy_validator::ValidationResult,
    };

    struct MockAgent(IntentDeployment);
    impl IIntentAgent for MockAgent {
        fn propose(&self, request: &IntentRequest) -> ContractResult<IntentProposal> {
            Ok(IntentProposal {
                workload_type: WorkloadType::Development,
                confidence: 0.9,
                reason: "mock".into(),
                expires_at_unix_ms: request.submitted_at_unix_ms + 1000,
            })
        }
        fn deployment(&self) -> IntentDeployment {
            self.0
        }
        fn name(&self) -> &'static str {
            "mock"
        }
    }
    struct MockEngine;
    impl IPolicyEngine for MockEngine {
        fn generate(
            &self,
            _snapshot: &ComputerSnapshot,
            context: &ContextSnapshot,
        ) -> ResourcePolicy {
            ResourcePolicy {
                policy_id: format!("intent-{:?}", context.workload_type),
                priority: PolicyPriority::Normal,
                ..Default::default()
            }
        }
        fn name(&self) -> &'static str {
            "mock"
        }
    }
    struct MockValidator(bool);
    impl IPolicyValidator for MockValidator {
        fn validate(&self, _policy: &ResourcePolicy) -> ValidationResult {
            if self.0 {
                ValidationResult::accept()
            } else {
                ValidationResult {
                    accepted: false,
                    issues: vec![],
                }
            }
        }
        fn name(&self) -> &'static str {
            "mock"
        }
    }
    fn request() -> IntentRequest {
        IntentRequest {
            application_id: ApplicationId::new("app.intent"),
            text: "prepare my editor".into(),
            submitted_at_unix_ms: 10,
        }
    }

    #[test]
    fn local_intent_must_pass_existing_validator() {
        let agent = MockAgent(IntentDeployment::Local);
        let policy = validated_intent_policy(
            &agent,
            &request(),
            IntentPolicyContext {
                profile: &UserProfile::default(),
                snapshot: &ComputerSnapshot::default(),
                context: &ContextSnapshot::default(),
                now_unix_ms: 20,
            },
            &MockEngine,
            &MockValidator(true),
        )
        .unwrap();
        assert!(policy.policy_id.contains("Development"));
        assert!(validated_intent_policy(
            &agent,
            &request(),
            IntentPolicyContext {
                profile: &UserProfile::default(),
                snapshot: &ComputerSnapshot::default(),
                context: &ContextSnapshot::default(),
                now_unix_ms: 20,
            },
            &MockEngine,
            &MockValidator(false),
        )
        .is_err());
    }

    #[test]
    fn cloud_intent_requires_consent() {
        let result = validated_intent_policy(
            &MockAgent(IntentDeployment::Cloud),
            &request(),
            IntentPolicyContext {
                profile: &UserProfile::default(),
                snapshot: &ComputerSnapshot::default(),
                context: &ContextSnapshot::default(),
                now_unix_ms: 20,
            },
            &MockEngine,
            &MockValidator(true),
        );
        assert!(matches!(result, Err(ContractError::PermissionDenied(_))));
    }
}
