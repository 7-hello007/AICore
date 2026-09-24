use aicore_ai::{OllamaIntentAgent, OllamaIntentConfig};
use aicore_contracts::{
    ai::intent::IntentRequest, application::id::ApplicationId, traits::intent_agent::IIntentAgent,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = OllamaIntentAgent::new(OllamaIntentConfig::from_env()?)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let proposal = agent.propose(&IntentRequest {
        application_id: ApplicationId::new("aicore.local_intent_smoke"),
        text: "I am compiling a Rust project in my editor".into(),
        submitted_at_unix_ms: now,
    })?;
    println!("{}", serde_json::to_string(&proposal)?);
    Ok(())
}
