use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use aicore_contracts::{
    ai::intent::{IntentDeployment, IntentProposal, IntentRequest},
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    traits::intent_agent::IIntentAgent,
};
use serde::Deserialize;

/// Configuration belongs to the provider, so changing a local model never
/// changes Core or the policy engine. Only loopback endpoints are accepted.
#[derive(Debug, Clone)]
pub struct OllamaIntentConfig {
    pub model: String,
    pub host: String,
    pub port: u16,
    pub timeout: Duration,
}

impl OllamaIntentConfig {
    pub fn from_env() -> ContractResult<Self> {
        let config = Self {
            model: std::env::var("AICORE_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".into()),
            host: std::env::var("AICORE_OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: std::env::var("AICORE_OLLAMA_PORT")
                .ok()
                .map(|value| value.parse())
                .transpose()
                .map_err(|_| ContractError::ConfigInvalid("invalid Ollama port".into()))?
                .unwrap_or(11434),
            timeout: Duration::from_millis(
                std::env::var("AICORE_OLLAMA_TIMEOUT_MS")
                    .ok()
                    .map(|value| value.parse())
                    .transpose()
                    .map_err(|_| ContractError::ConfigInvalid("invalid Ollama timeout".into()))?
                    .unwrap_or(30_000),
            ),
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> ContractResult<()> {
        if !matches!(self.host.as_str(), "127.0.0.1" | "localhost" | "::1")
            || self.model.trim().is_empty()
            || self.model.len() > 256
            || self.port == 0
            || self.timeout.is_zero()
        {
            return Err(ContractError::ConfigInvalid(
                "Ollama requires a local endpoint, model and positive timeout".into(),
            ));
        }
        Ok(())
    }
}

pub struct OllamaIntentAgent {
    config: OllamaIntentConfig,
}

impl OllamaIntentAgent {
    pub fn new(config: OllamaIntentConfig) -> ContractResult<Self> {
        config.validate()?;
        Ok(Self { config })
    }
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(Deserialize)]
struct ModelProposal {
    workload_type: WorkloadType,
    confidence: f32,
    reason: String,
}

impl IIntentAgent for OllamaIntentAgent {
    fn propose(&self, request: &IntentRequest) -> ContractResult<IntentProposal> {
        if request.text.trim().is_empty() || request.text.len() > 4096 {
            return Err(ContractError::ContractInvalid("invalid intent text".into()));
        }
        let prompt = format!(
            "Classify the following computer task. Return only JSON with workload_type, confidence, reason. workload_type must be one of development, gaming, video_editing, ai_inference, rendering, compilation, file_processing, video_conference, general_office, background_task, unknown. Confidence must be a number from 0 to 1. Do not give computer commands. Task: {}",
            request.text
        );
        let body = serde_json::json!({
            "model": self.config.model,
            "prompt": prompt,
            "format": "json",
            "stream": false,
            "options": {"temperature": 0}
        })
        .to_string();
        let address = (self.config.host.as_str(), self.config.port)
            .to_socket_addrs()
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?
            .next()
            .ok_or_else(|| ContractError::AiInferenceFailed("Ollama address unavailable".into()))?;
        let mut stream = TcpStream::connect_timeout(&address, self.config.timeout)
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .and_then(|_| stream.set_write_timeout(Some(self.config.timeout)))
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        write!(
            stream,
            "POST /api/generate HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        let mut response = Vec::new();
        stream
            .take(256 * 1024 + 1)
            .read_to_end(&mut response)
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        if response.len() > 256 * 1024 {
            return Err(ContractError::AiInferenceFailed(
                "Ollama response too large".into(),
            ));
        }
        let split = response
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .ok_or_else(|| {
                ContractError::AiInferenceFailed("invalid Ollama HTTP response".into())
            })?;
        let headers = std::str::from_utf8(&response[..split])
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        if !headers.starts_with("HTTP/1.1 200 ") && !headers.starts_with("HTTP/1.0 200 ") {
            return Err(ContractError::AiInferenceFailed(format!(
                "Ollama HTTP status: {}",
                headers.lines().next().unwrap_or("unknown")
            )));
        }
        let payload = &response[split + 4..];
        let decoded = if headers
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            decode_chunks(payload)?
        } else {
            payload.to_vec()
        };
        let generated: GenerateResponse = serde_json::from_slice(&decoded)
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        let proposal: ModelProposal = serde_json::from_str(&generated.response)
            .map_err(|error| ContractError::AiInferenceFailed(error.to_string()))?;
        if proposal.reason.len() > 512 || !proposal.confidence.is_finite() {
            return Err(ContractError::AiInferenceFailed(
                "invalid Ollama proposal".into(),
            ));
        }
        Ok(IntentProposal {
            workload_type: proposal.workload_type,
            confidence: proposal.confidence,
            reason: proposal.reason,
            expires_at_unix_ms: request.submitted_at_unix_ms.saturating_add(30_000),
        })
    }

    fn deployment(&self) -> IntentDeployment {
        IntentDeployment::Local
    }

    fn name(&self) -> &'static str {
        "ollama_intent_agent"
    }
}

fn decode_chunks(mut bytes: &[u8]) -> ContractResult<Vec<u8>> {
    let mut decoded = Vec::new();
    loop {
        let end = bytes
            .windows(2)
            .position(|pair| pair == b"\r\n")
            .ok_or_else(|| ContractError::AiInferenceFailed("invalid HTTP chunk".into()))?;
        let size = std::str::from_utf8(&bytes[..end])
            .ok()
            .and_then(|text| text.split(';').next())
            .and_then(|text| usize::from_str_radix(text, 16).ok())
            .ok_or_else(|| ContractError::AiInferenceFailed("invalid HTTP chunk size".into()))?;
        bytes = &bytes[end + 2..];
        if size == 0 {
            return Ok(decoded);
        }
        if bytes.len() < size + 2 || &bytes[size..size + 2] != b"\r\n" {
            return Err(ContractError::AiInferenceFailed(
                "truncated HTTP chunk".into(),
            ));
        }
        decoded.extend_from_slice(&bytes[..size]);
        bytes = &bytes[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::application::id::ApplicationId;
    use std::net::TcpListener;

    #[test]
    fn loopback_provider_parses_typed_proposal() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let count = socket.read(&mut chunk).unwrap();
                request.extend_from_slice(&chunk[..count]);
                let Some(split) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..split]);
                let length: usize = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if request.len() >= split + 4 + length {
                    assert!(headers.contains("/api/generate"));
                    break;
                }
            }
            let body = serde_json::json!({"response": "{\"workload_type\":\"development\",\"confidence\":0.9,\"reason\":\"editor\"}"}).to_string();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let agent = OllamaIntentAgent::new(OllamaIntentConfig {
            model: "test".into(),
            host: "127.0.0.1".into(),
            port,
            timeout: Duration::from_secs(2),
        })
        .unwrap();
        let proposal = agent
            .propose(&IntentRequest {
                application_id: ApplicationId::new("app.test"),
                text: "open editor".into(),
                submitted_at_unix_ms: 100,
            })
            .unwrap();
        server.join().unwrap();
        assert_eq!(proposal.workload_type, WorkloadType::Development);
        assert_eq!(proposal.expires_at_unix_ms, 30_100);
    }

    #[test]
    fn non_loopback_endpoint_is_rejected() {
        assert!(OllamaIntentAgent::new(OllamaIntentConfig {
            model: "test".into(),
            host: "example.com".into(),
            port: 11434,
            timeout: Duration::from_secs(1),
        })
        .is_err());
    }
}
