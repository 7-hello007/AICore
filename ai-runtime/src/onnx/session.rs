//! ONNX Session 抽象。
//!
//! ONNX Session contract。
//! 当前 V3.2 release baseline 使用 dependency-free Mock session 进行契约测试；
//! 真实 ONNX provider 是可插拔实现，不属于 Core contract。

use std::path::Path;
use std::sync::Arc;

use aicore_contracts::ai::runtime::{
    InferenceLabel, OnnxModelSpec, TensorDType, TensorInput, TensorSchema,
};

/// 一次 ONNX 推理会话。
pub trait IOnnxSession: Send + Sync {
    fn run(&self, inputs: &[TensorInput]) -> Result<Vec<InferenceLabel>, String>;
    fn input_schemas(&self) -> Vec<TensorSchema>;
    fn output_schemas(&self) -> Vec<TensorSchema>;
    fn provider_name(&self) -> &'static str;
}

/// Session 工厂。
pub trait IOnnxSessionFactory: Send + Sync {
    fn create_session(
        &self,
        spec: &OnnxModelSpec,
        model_path: &Path,
    ) -> Result<Arc<dyn IOnnxSession>, String>;

    fn factory_name(&self) -> &'static str;
}

// ============================================================
// Mock 实现（默认）
// ============================================================

pub struct MockOnnxSession {
    input_schemas: Vec<TensorSchema>,
    output_schemas: Vec<TensorSchema>,
    labels: Vec<String>,
}

impl MockOnnxSession {
    pub fn new(
        input_schemas: Vec<TensorSchema>,
        output_schemas: Vec<TensorSchema>,
        labels: Vec<String>,
    ) -> Self {
        Self {
            input_schemas,
            output_schemas,
            labels,
        }
    }

    pub fn default_test() -> Self {
        Self::new(
            vec![TensorSchema {
                name: "input".to_string(),
                shape: vec![1, 4],
                dtype: TensorDType::F32,
            }],
            vec![TensorSchema {
                name: "output".to_string(),
                shape: vec![1, 4],
                dtype: TensorDType::F32,
            }],
            vec![
                "boost_cpu".to_string(),
                "boost_gpu".to_string(),
                "boost_npu".to_string(),
                "keep_base".to_string(),
            ],
        )
    }
}

impl IOnnxSession for MockOnnxSession {
    fn run(&self, inputs: &[TensorInput]) -> Result<Vec<InferenceLabel>, String> {
        if inputs.len() != self.input_schemas.len() {
            return Err(format!(
                "input count {} != expected {}",
                inputs.len(),
                self.input_schemas.len()
            ));
        }
        for (i, (input, schema)) in inputs.iter().zip(self.input_schemas.iter()).enumerate() {
            if input.name != schema.name {
                return Err(format!(
                    "input[{}] name `{}` != `{}`",
                    i, input.name, schema.name
                ));
            }
            if input.shape != schema.shape {
                return Err(format!(
                    "input[{}] shape {:?} != {:?}",
                    i, input.shape, schema.shape
                ));
            }
            let expected_len: usize = schema.shape.iter().map(|d| *d as usize).product();
            if input.data.len() != expected_len {
                return Err(format!(
                    "input[{}] data length {} != {}",
                    i,
                    input.data.len(),
                    expected_len
                ));
            }
        }

        let primary = &inputs[0].data;
        let total_abs: f32 = primary.iter().map(|x| x.abs()).sum();
        let mut labels: Vec<InferenceLabel> = self
            .labels
            .iter()
            .enumerate()
            .map(|(i, name)| InferenceLabel {
                label: name.clone(),
                score: total_abs / (i as f32 + 1.0),
            })
            .collect();

        let sum: f32 = labels.iter().map(|l| l.score).sum();
        if sum > 0.0 {
            for l in labels.iter_mut() {
                l.score /= sum;
            }
        }
        labels.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(labels)
    }

    fn input_schemas(&self) -> Vec<TensorSchema> {
        self.input_schemas.clone()
    }

    fn output_schemas(&self) -> Vec<TensorSchema> {
        self.output_schemas.clone()
    }

    fn provider_name(&self) -> &'static str {
        "mock"
    }
}

/// Mock session 工厂。
///
/// - 默认总是成功（路径必须存在）。
/// - `failing(reason)` 版本总是返回 Err，用于测试 fallback。
pub struct MockOnnxSessionFactory {
    session: Arc<dyn IOnnxSession>,
    fail_reason: Option<String>,
    /// 若为 true，即使路径存在也失败。
    skip_path_check: bool,
}

impl MockOnnxSessionFactory {
    pub fn new(session: Arc<dyn IOnnxSession>) -> Self {
        Self {
            session,
            fail_reason: None,
            skip_path_check: false,
        }
    }

    pub fn with_default_test_session() -> Self {
        Self::new(Arc::new(MockOnnxSession::default_test()))
    }

    pub fn failing(reason: impl Into<String>) -> Self {
        Self {
            session: Arc::new(MockOnnxSession::default_test()),
            fail_reason: Some(reason.into()),
            skip_path_check: true,
        }
    }
}

impl IOnnxSessionFactory for MockOnnxSessionFactory {
    fn create_session(
        &self,
        _spec: &OnnxModelSpec,
        model_path: &Path,
    ) -> Result<Arc<dyn IOnnxSession>, String> {
        if let Some(reason) = &self.fail_reason {
            return Err(format!("mock factory rejects: {}", reason));
        }
        if !self.skip_path_check && !model_path.exists() {
            return Err(format!("model file not found: {}", model_path.display()));
        }
        Ok(self.session.clone())
    }

    fn factory_name(&self) -> &'static str {
        "mock_onnx_factory"
    }
}
