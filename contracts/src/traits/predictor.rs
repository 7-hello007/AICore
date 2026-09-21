use crate::errors::ContractResult;
use crate::prediction::{input::PredictionInput, output::Prediction};

/// 短期预测器抽象。
///
/// 用途：
/// - 预测下一轮 tick 的 workload。
/// - 预测失败必须返回 Err，让上层回退到当前观测。
pub trait IPredictor: Send + Sync {
    /// 执行预测。
    ///
    /// 数据不足 / 无法预测时返回 Err，让调用方回退。
    fn predict(&self, input: &PredictionInput) -> ContractResult<Prediction>;

    /// 预测器名称。
    fn name(&self) -> &'static str;

    /// 预测窗口，毫秒。
    fn horizon_ms(&self) -> u64;
}
