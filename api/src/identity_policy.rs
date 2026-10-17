use std::collections::HashMap;
use std::sync::RwLock;

use aicore_contracts::{
    application::id::ApplicationId,
    errors::{ContractError, ContractResult},
};
use serde::{Deserialize, Serialize};

/// Actions are authorized individually after the transport authenticates the
/// peer. A registration request cannot grant itself additional actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationAction {
    Register,
    Unregister,
    SubmitTask,
    DeclareTask,
    ClearTask,
    SubmitNativeTask,
    ReadNativeTask,
    CancelNativeTask,
    ProposeIntent,
    PrepareTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedApplication {
    pub application_id: ApplicationId,
    /// SHA-256 of a platform-resolved executable artifact, never client input.
    pub artifact_sha256: String,
    /// Transport-resolved OS principal: Windows SID or `uid:<number>`.
    /// Empty preserves legacy artifact-only policies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_principals: Vec<String>,
    pub allowed_actions: Vec<ApplicationAction>,
}

/// Operator-owned pin registry. Empty or missing policy fails closed.
#[derive(Default)]
pub struct ApplicationIdentityPolicy {
    pins: RwLock<HashMap<ApplicationId, ApprovedApplication>>,
}

impl ApplicationIdentityPolicy {
    pub fn try_approval(&self, id: &ApplicationId) -> ContractResult<Option<ApprovedApplication>> {
        Ok(self
            .pins
            .try_read()
            .map_err(|_| ContractError::ResourceUnavailable("application policy busy".into()))?
            .get(id)
            .cloned())
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pin(&self, approval: ApprovedApplication) -> ContractResult<()> {
        if !approval.application_id.is_valid()
            || approval.artifact_sha256.len() != 64
            || !approval
                .artifact_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || approval.allowed_actions.is_empty()
            || approval
                .allowed_principals
                .iter()
                .any(|p| p.trim().is_empty() || p.len() > 184)
        {
            return Err(ContractError::ContractInvalid(
                "invalid application pin".into(),
            ));
        }
        self.pins
            .write()
            .map_err(|_| ContractError::LockPoisoned("application pin lock poisoned".into()))?
            .insert(approval.application_id.clone(), approval);
        Ok(())
    }

    pub fn revoke(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        Ok(self
            .pins
            .write()
            .map_err(|_| ContractError::LockPoisoned("application pin lock poisoned".into()))?
            .remove(application_id)
            .is_some())
    }

    pub fn authorize(
        &self,
        application_id: &ApplicationId,
        artifact_sha256: &str,
        action: ApplicationAction,
    ) -> ContractResult<()> {
        self.authorize_identity(application_id, artifact_sha256, None, action)
    }

    pub fn authorize_identity(
        &self,
        application_id: &ApplicationId,
        artifact_sha256: &str,
        principal: Option<&str>,
        action: ApplicationAction,
    ) -> ContractResult<()> {
        let pins = self
            .pins
            .read()
            .map_err(|_| ContractError::LockPoisoned("application pin lock poisoned".into()))?;
        let approved = pins.get(application_id).ok_or_else(|| {
            ContractError::PermissionDenied("application has no operator pin".into())
        })?;
        if !approved
            .artifact_sha256
            .eq_ignore_ascii_case(artifact_sha256)
            || !approved.allowed_actions.contains(&action)
            || (!approved.allowed_principals.is_empty()
                && !principal.is_some_and(|p| {
                    approved
                        .allowed_principals
                        .iter()
                        .any(|allowed| allowed == p)
                }))
        {
            return Err(ContractError::PermissionDenied(
                "application artifact or action is not approved".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_restriction_is_additive_and_missing_principal_denied() {
        let policy = ApplicationIdentityPolicy::new();
        let id = ApplicationId::new("test.sid");
        let digest = "b".repeat(64);
        policy
            .pin(ApprovedApplication {
                application_id: id.clone(),
                artifact_sha256: digest.clone(),
                allowed_principals: vec!["S-1-5-21-123".into()],
                allowed_actions: vec![ApplicationAction::Register],
            })
            .unwrap();
        assert!(policy
            .authorize(&id, &digest, ApplicationAction::Register)
            .is_err());
        assert!(policy
            .authorize_identity(
                &id,
                &digest,
                Some("S-1-5-21-999"),
                ApplicationAction::Register
            )
            .is_err());
        assert!(policy
            .authorize_identity(
                &id,
                &digest,
                Some("S-1-5-21-123"),
                ApplicationAction::Register
            )
            .is_ok());
        assert!(policy
            .authorize_identity(
                &id,
                &"c".repeat(64),
                Some("S-1-5-21-123"),
                ApplicationAction::Register
            )
            .is_err());
    }

    #[test]
    fn artifact_and_action_are_both_required_and_revocable() {
        let policy = ApplicationIdentityPolicy::new();
        let id = ApplicationId::new("app.editor");
        let digest = "a".repeat(64);
        assert!(policy
            .authorize(&id, &digest, ApplicationAction::Register)
            .is_err());
        policy
            .pin(ApprovedApplication {
                application_id: id.clone(),
                artifact_sha256: digest.clone(),
                allowed_principals: Vec::new(),
                allowed_actions: vec![ApplicationAction::Register, ApplicationAction::SubmitTask],
            })
            .unwrap();
        assert!(policy
            .authorize(&id, &digest, ApplicationAction::Register)
            .is_ok());
        assert!(policy
            .authorize(&id, &digest, ApplicationAction::CancelNativeTask)
            .is_err());
        assert!(policy
            .authorize(&id, &"b".repeat(64), ApplicationAction::SubmitTask)
            .is_err());
        assert!(policy.revoke(&id).unwrap());
        assert!(policy
            .authorize(&id, &digest, ApplicationAction::SubmitTask)
            .is_err());
    }
}
