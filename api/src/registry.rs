use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    application::{id::ApplicationId, registration::ApplicationRegistration},
    errors::{ContractError, ContractResult},
};

/// 应用注册中心。
///
/// 记录当前已注册的应用。未注册应用不允许提交 TaskContract。
#[derive(Default)]
pub struct ApplicationRegistry {
    apps: RwLock<HashMap<ApplicationId, ApplicationRegistration>>,
}

impl ApplicationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册应用。若已存在则覆盖（视为版本更新）。
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

    /// 注销应用。返回该应用之前是否存在。
    pub fn unregister(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        let mut apps = self
            .apps
            .write()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(apps.remove(application_id).is_some())
    }

    /// 查询应用注册信息。
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

    /// 检查应用是否已注册。
    pub fn is_registered(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        Ok(self.get(application_id)?.is_some())
    }

    /// 列出所有已注册应用。
    pub fn list(&self) -> ContractResult<Vec<ApplicationRegistration>> {
        let apps = self
            .apps
            .read()
            .map_err(|_| ContractError::AdapterUnavailable("registry lock poisoned".into()))?;
        Ok(apps.values().cloned().collect())
    }

    /// 已注册应用数量。
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
