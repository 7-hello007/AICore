/// Telemetry sample constraint.
///
/// For Adapter to decide "should we skip this collection"?
/// Adapter does not do scheduling; only does inspection advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SamplingConstraints {
    /// Minimum interval, millisecond.
    pub min_interval_ms: u64,
}

impl SamplingConstraints {
    pub fn new(min_interval_ms: u64) -> Self {
        Self { min_interval_ms }
    }

    /// For example, `since_last_ms` (how many milliseconds away from this time) has been used for this project.
    /// Whether or not to skip this collection.
    pub fn should_skip(&self, since_last_ms: u64) -> bool {
        since_last_ms < self.min_interval_ms
    }
}
