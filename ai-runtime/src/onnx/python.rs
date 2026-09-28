use super::session::{IOnnxSession, IOnnxSessionFactory};
use aicore_contracts::ai::runtime::{InferenceLabel, OnnxModelSpec, TensorInput, TensorSchema};
use serde::Deserialize;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

/// Explicit project-local Python ONNX Runtime bridge. It never installs or
/// downloads dependencies; every invocation is bounded and killable.
#[derive(Clone)]
pub struct PythonOnnxSessionFactory {
    pub python: PathBuf,
    pub runner: PathBuf,
    pub packages: PathBuf,
    pub timeout: Duration,
    /// Explicit accelerator ordinal from configuration; never inferred from vendor name.
    pub device_id: Option<u32>,
    /// Optional preinstalled EP library; registration is subprocess-local.
    pub provider_library: Option<PathBuf>,
}

impl PythonOnnxSessionFactory {
    fn invoke(&self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        if self.timeout.is_zero() {
            return Err("ONNX timeout must be positive".into());
        }
        let started = Instant::now();
        let input = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        if input.len() > 262_144 {
            return Err("ONNX input too large".into());
        }
        let mut child = Command::new(&self.python)
            .arg("-I")
            .arg("-S")
            .arg(&self.runner)
            .arg(&self.packages)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        // Drain both outputs and write input concurrently: a stalled reader
        // must not keep write_all outside the subprocess timeout.
        let mut stdin = child.stdin.take().ok_or("ONNX stdin missing")?;
        let stdout = child.stdout.take().ok_or("ONNX stdout missing")?;
        let stderr = child.stderr.take().ok_or("ONNX stderr missing")?;
        fn capture(mut stream: impl Read) -> std::io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let n = stream.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                if bytes.len() < 262_145 {
                    let remaining = 262_145 - bytes.len();
                    bytes.extend_from_slice(&buffer[..n.min(remaining)]);
                }
            }
            Ok(bytes)
        }
        let out = std::thread::spawn(move || capture(stdout));
        let err = std::thread::spawn(move || capture(stderr));
        let writer = std::thread::spawn(move || stdin.write_all(&input));
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if started.elapsed() < self.timeout => {
                    std::thread::sleep(Duration::from_millis(5))
                },
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err("ONNX subprocess timed out".to_string());
                },
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(error.to_string());
                },
            }
        };
        let output = out
            .join()
            .map_err(|_| "ONNX stdout reader panicked")?
            .map_err(|e| e.to_string())?;
        let errors = err
            .join()
            .map_err(|_| "ONNX stderr reader panicked")?
            .map_err(|e| e.to_string())?;
        let written = writer.join().map_err(|_| "ONNX stdin writer panicked")?;
        if !status?.success() {
            return Err(format!(
                "ONNX process failed: {}",
                String::from_utf8_lossy(&errors)
            ));
        }
        written.map_err(|e| e.to_string())?;
        if output.len() > 262_144 {
            return Err("ONNX output too large".into());
        }
        serde_json::from_slice(&output).map_err(|e| e.to_string())
    }
}

struct PythonSession {
    factory: PythonOnnxSessionFactory,
    path: PathBuf,
    spec: OnnxModelSpec,
    provider: &'static str,
}

impl IOnnxSessionFactory for PythonOnnxSessionFactory {
    fn create_session(
        &self,
        spec: &OnnxModelSpec,
        model_path: &Path,
    ) -> Result<Arc<dyn IOnnxSession>, String> {
        #[derive(Deserialize)]
        struct Description {
            inputs: Vec<TensorSchema>,
            outputs: Vec<TensorSchema>,
        }
        let provider = match spec
            .preferred_provider
            .as_deref()
            .unwrap_or("CPUExecutionProvider")
        {
            "CPUExecutionProvider" => "CPUExecutionProvider",
            "DmlExecutionProvider" => "DmlExecutionProvider",
            "VitisAIExecutionProvider" => "VitisAIExecutionProvider",
            _ => return Err("unsupported explicit ONNX provider".into()),
        };
        if provider == "DmlExecutionProvider" && self.device_id.is_none() {
            return Err("DirectML requires explicit device ordinal".into());
        }
        let path = model_path.canonicalize().map_err(|e| e.to_string())?;
        let response = self.invoke(&serde_json::json!({"mode":"describe", "model": path,"provider":provider,"device_id":self.device_id,"provider_library":self.provider_library}))?;
        let description: Description =
            serde_json::from_value(response).map_err(|e| e.to_string())?;
        if description.inputs != spec.input_schema || description.outputs != spec.output_schema {
            return Err("ONNX actual schemas differ from declared schemas".into());
        }
        Ok(Arc::new(PythonSession {
            factory: self.clone(),
            path,
            spec: spec.clone(),
            provider,
        }))
    }
    fn factory_name(&self) -> &'static str {
        "python_onnx_explicit_factory"
    }
}

impl IOnnxSession for PythonSession {
    fn run(&self, inputs: &[TensorInput]) -> Result<Vec<InferenceLabel>, String> {
        let tensors: Vec<_> = inputs
            .iter()
            .map(|t| serde_json::json!({"name":t.name, "shape":t.shape, "data":t.data}))
            .collect();
        serde_json::from_value(
            self.factory.invoke(
                &serde_json::json!({"mode":"infer", "model":self.path, "inputs":tensors,"provider":self.provider,"device_id":self.factory.device_id,"provider_library":self.factory.provider_library}),
            )?,
        )
        .map_err(|e| e.to_string())
    }
    fn input_schemas(&self) -> Vec<TensorSchema> {
        self.spec.input_schema.clone()
    }
    fn output_schemas(&self) -> Vec<TensorSchema> {
        self.spec.output_schema.clone()
    }
    fn provider_name(&self) -> &'static str {
        self.provider
    }
}
