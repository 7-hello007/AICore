use crate::errors::ContractResult;
use crate::telemetry::{constraints::SamplingConstraints, ComputerSnapshot};

/// Telemetry interfaces that must be implemented by every platform adapter.
pub trait ITelemetryProvider: Send + Sync {
    /// Collect a complete system state snapshot.
    fn collect(&self) -> ContractResult<ComputerSnapshot>;

    /// Read one explicitly identified process without assuming it appears in
    /// the bounded top-process list. Unsupported collectors fail explicitly.
    fn collect_process(
        &self,
        _pid: u32,
    ) -> ContractResult<crate::telemetry::process::ProcessSnapshot> {
        Err(crate::errors::ContractError::NotSupported(
            "targeted process telemetry unavailable".into(),
        ))
    }

    /// Returns the name of the Provider.
    fn platform_name(&self) -> &'static str;

    /// Returns the list of capabilities supported.
    fn capabilities(&self) -> Vec<&'static str>;

    /// Returns the sampling constraint. Default is unlimited.
    fn sampling_constraints(&self) -> SamplingConstraints {
        SamplingConstraints::default()
    }
}
