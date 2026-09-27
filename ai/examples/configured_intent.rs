use aicore_contracts::{
    ai::intent::IntentRequest, application::id::ApplicationId, traits::intent_agent::IIntentAgent,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let agent = aicore_ai::configured_intent_agent(&project)?.ok_or("intent disabled")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let proposal = agent.propose(&IntentRequest {
        application_id: ApplicationId::new("example.local"),
        text: "compiling rust".into(),
        submitted_at_unix_ms: now,
    })?;
    println!("{}", serde_json::to_string(&proposal)?);
    Ok(())
}
