use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    application::{id::ApplicationId, registration::ApplicationRegistration},
    errors::{ContractError, ContractResult},
};

/// Application registration centres.
///
/// Records the current registered application. Unregistered applications do not allow TaskContract to be submitted.
#[derive(Default)]
pub struct ApplicationRegistry {
    apps: RwLock<HashMap<ApplicationId, ApplicationRegistration>>,
}

impl ApplicationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register application. Overwrite if it already exists (as version update).
    pub fn register(
        &self,
        application_id: ApplicationId,
        name: impl Into<String>,
        version: impl Into<String>,
        capabilities: Vec<String>,
    ) -> ContractResult<ApplicationRegistration> {
        if !application_id.is_valid() {
            return Err(ContractError::ContractInvalid(format!(
                "invalid application_id: {:?}",
                application_id
            )));
        }

        let registration = ApplicationRegistration {
            application_id: application_id.clone(),
            name: name.into(),
            version: version.into(),
            registered_at_unix_secs: now_unix_secs(),
            capabilities,
        };

        let mut apps = self
            .apps
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        apps.insert(application_id, registration.clone());
        Ok(registration)
    }

    /// Write-off application.
    pub fn unregister(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        let mut apps = self
            .apps
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(apps.remove(application_id).is_some())
    }

    /// Query application registration information.
    pub fn get(
        &self,
        application_id: &ApplicationId,
    ) -> ContractResult<Option<ApplicationRegistration>> {
        let apps = self
            .apps
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(apps.get(application_id).cloned())
    }

    /// Checks if application is registered.
    pub fn is_registered(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        Ok(self.get(application_id)?.is_some())
    }

    /// Lists all registered applications.
    pub fn list(&self) -> ContractResult<Vec<ApplicationRegistration>> {
        let apps = self
            .apps
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(apps.values().cloned().collect())
    }

    /// Number of registered applications.
    pub fn count(&self) -> ContractResult<usize> {
        let apps = self
            .apps
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(apps.len())
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
