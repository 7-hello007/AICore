use crate::policy::resource_policy::ResourcePolicy;

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    pub accepted: bool,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    pub fn accept() -> Self {
        Self {
            accepted: true,
            issues: Vec::new(),
        }
    }
}

/// Polycy Validator interface.
///
/// Every policy must pass through this interface before reaching an executor.
pub trait IPolicyValidator: Send + Sync {
    fn validate(&self, policy: &ResourcePolicy) -> ValidationResult;
    fn name(&self) -> &'static str;
}
