pub mod lifecycle;
pub mod mock;
pub mod upgrade;
pub mod windows;

pub use lifecycle::ServiceLifecycle;
pub use mock::MockServiceController;
pub use upgrade::{UpgradeOutcome, UpgradePlan};
pub use windows::WindowsServiceController;
