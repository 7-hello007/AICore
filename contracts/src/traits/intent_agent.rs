use crate::{
    ai::intent::{IntentDeployment, IntentProposal, IntentRequest},
    errors::ContractResult,
};

/// Intent provider returns a semantic proposal, never an executable policy or
/// direct OS command. Its output must pass policy generation and validation.
pub trait IIntentAgent: Send + Sync {
    fn propose(&self, request: &IntentRequest) -> ContractResult<IntentProposal>;
    fn deployment(&self) -> IntentDeployment;
    fn name(&self) -> &'static str;
}
