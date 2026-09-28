use crate::capability::entry::CapabilityEntry;

/// Capability providers are abstract.
///
/// Any adapter, provider, or executor can implement this trait.
/// It is a way to report its actual supported capabilities.
pub trait ICapabilityProvider: Send + Sync {
    /// Returns the list of capabilities that are actually supported in the current environment.
    ///
    /// Implementation requirements:
    /// - Must return at least one capability.
    /// - Non-`Supported`/`ReadOnly` capabilities must be accompanied by detail.
    /// - The list of capabilities should be a definitive snapshot; do not modify the internal state on call.
    fn capabilities_detailed(&self) -> Vec<CapabilityEntry>;

    /// Component name, e.g. `linux_capability`/`windows_capability`.
    fn component_name(&self) -> &'static str;

    /// Easy way: return only to the name of the ability.
    fn capability_names(&self) -> Vec<String> {
        self.capabilities_detailed()
            .into_iter()
            .map(|c| c.name)
            .collect()
    }

    /// Easy approach: query individual capabilities.
    fn capability(&self, name: &str) -> Option<CapabilityEntry> {
        self.capabilities_detailed()
            .into_iter()
            .find(|c| c.name == name)
    }
}
