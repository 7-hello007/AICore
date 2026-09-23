pub mod activation;
pub mod discovery;
pub mod host;
pub mod isolation;
pub mod loader;
pub mod registry;
pub mod sandbox;
pub mod signature;
pub mod trust;

pub use activation::{
    ActivationError, IManagedPluginHost, PluginActivationManager, PluginActivationPin,
    PluginActivationPolicy,
};
pub use discovery::{
    discover_from_dir, register_discovered, DiscoveredPlugin, DiscoveryReport, RejectedPlugin,
};
pub use host::{HostRequest, HostResponse, SubprocessPluginHost};
pub use isolation::PluginIsolation;
pub use loader::{DynamicPluginLoader, LoadError, LoadedPlugin};
pub use registry::{PluginEntry, PluginRegistry, RegistryError};
pub use sandbox::{run_with_timeout, SandboxError};
pub use signature::{SignatureVerifier, SignatureVerifyError};
pub use trust::{TrustEntry, TrustError, TrustStore, TrustVerification};
