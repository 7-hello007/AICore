use crate::errors::ContractResult;
use crate::prediction::{input::PredictionInput, output::Prediction};

/// Short-term predictors are abstract.
///
/// Purpose:
/// - Forecast the next round of tick workloads.
/// - The prediction failure must return to Err and allow the upper layer to retreat to current observations.
pub trait IPredictor: Send + Sync {
    /// Implementation projections.
    ///
    /// Insufficient data / Unpredictable return Err to reverse caller.
    fn predict(&self, input: &PredictionInput) -> ContractResult<Prediction>;

    /// Projector name.
    fn name(&self) -> &'static str;

    /// Forecast window, millisecond.
    fn horizon_ms(&self) -> u64;
}
