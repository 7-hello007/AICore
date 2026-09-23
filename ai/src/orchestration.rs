use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use aicore_contracts::{
    ai::intent::{IntentDeployment, IntentProposal, IntentRequest},
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    memory::profile::UserProfile,
    traits::intent_agent::IIntentAgent,
};

use crate::intent::BoundedIntentAgent;

#[derive(Debug, Clone)]
pub struct AgentConsensus {
    pub proposal: IntentProposal,
    pub contributors: usize,
}

/// Coordinates at most four bounded intent agents. Disagreement or missing
/// consent produces no proposal; no agent can directly execute a policy.
pub struct IntentAgentCoordinator {
    agents: Vec<BoundedIntentAgent>,
}

impl IntentAgentCoordinator {
    pub fn new(
        agents: Vec<Arc<dyn IIntentAgent>>,
        per_agent_deadline: Duration,
    ) -> ContractResult<Self> {
        if agents.is_empty() || agents.len() > 4 || per_agent_deadline.is_zero() {
            return Err(ContractError::ContractInvalid(
                "intent coordinator requires 1..=4 bounded agents".into(),
            ));
        }
        let agents = agents
            .into_iter()
            .map(|agent| BoundedIntentAgent::new(agent, per_agent_deadline))
            .collect::<ContractResult<Vec<_>>>()?;
        Ok(Self { agents })
    }

    pub fn propose(
        &self,
        request: &IntentRequest,
        profile: &UserProfile,
        now_unix_ms: u64,
    ) -> ContractResult<AgentConsensus> {
        let mut votes: HashMap<WorkloadType, (f32, usize, IntentProposal)> = HashMap::new();
        for agent in &self.agents {
            if agent.deployment() == IntentDeployment::Cloud && !profile.can_use_cloud() {
                continue;
            }
            let proposal = match agent.propose(request) {
                Ok(proposal)
                    if proposal.workload_type != WorkloadType::Unknown
                        && proposal.confidence.is_finite()
                        && (0.6..=1.0).contains(&proposal.confidence)
                        && proposal.expires_at_unix_ms > now_unix_ms =>
                {
                    proposal
                },
                _ => continue,
            };
            let vote = votes
                .entry(proposal.workload_type)
                .or_insert((0.0, 0, proposal.clone()));
            vote.0 += proposal.confidence;
            vote.1 += 1;
            if proposal.confidence > vote.2.confidence {
                vote.2 = proposal;
            }
        }
        let mut ranked: Vec<_> = votes.into_values().collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        let Some((best_score, contributors, mut proposal)) = ranked.first().cloned() else {
            return Err(ContractError::AiInferenceFailed(
                "no consented, timely intent proposal".into(),
            ));
        };
        if ranked
            .get(1)
            .is_some_and(|runner_up| best_score - runner_up.0 < 0.1)
        {
            return Err(ContractError::AiInferenceFailed(
                "intent agents disagree".into(),
            ));
        }
        proposal.confidence = best_score / contributors as f32;
        Ok(AgentConsensus {
            proposal,
            contributors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::application::id::ApplicationId;

    struct MockAgent {
        deployment: IntentDeployment,
        workload: WorkloadType,
        confidence: f32,
    }
    impl IIntentAgent for MockAgent {
        fn propose(&self, request: &IntentRequest) -> ContractResult<IntentProposal> {
            Ok(IntentProposal {
                workload_type: self.workload,
                confidence: self.confidence,
                reason: "mock".into(),
                expires_at_unix_ms: request.submitted_at_unix_ms + 1000,
            })
        }
        fn deployment(&self) -> IntentDeployment {
            self.deployment
        }
        fn name(&self) -> &'static str {
            "mock"
        }
    }
    fn request() -> IntentRequest {
        IntentRequest {
            application_id: ApplicationId::new("app.test"),
            text: "mock".into(),
            submitted_at_unix_ms: 10,
        }
    }
    #[test]
    fn local_agents_agree_without_cloud_consent() {
        let agents: Vec<Arc<dyn IIntentAgent>> = vec![
            Arc::new(MockAgent {
                deployment: IntentDeployment::Local,
                workload: WorkloadType::Development,
                confidence: 0.8,
            }),
            Arc::new(MockAgent {
                deployment: IntentDeployment::Local,
                workload: WorkloadType::Development,
                confidence: 0.9,
            }),
            Arc::new(MockAgent {
                deployment: IntentDeployment::Cloud,
                workload: WorkloadType::Gaming,
                confidence: 1.0,
            }),
        ];
        let coordinator = IntentAgentCoordinator::new(agents, Duration::from_millis(50)).unwrap();
        let consensus = coordinator
            .propose(&request(), &UserProfile::default(), 20)
            .unwrap();
        assert_eq!(consensus.proposal.workload_type, WorkloadType::Development);
        assert_eq!(consensus.contributors, 2);
        assert!((consensus.proposal.confidence - 0.85).abs() < 0.001);
    }
}
