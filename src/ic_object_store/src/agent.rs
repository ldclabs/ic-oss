use ic_agent::{Agent, Identity};
use ic_oss_types::format_error;
use std::sync::Arc;

/// Creates and configures an IC agent with the given host URL and identity.
///
/// # Arguments
/// * `host` - The IC host URL (e.g., "https://ic0.app" or "http://localhost:4943")
/// * `identity` - Arc-wrapped identity for authentication
///
/// # Returns
/// Result containing the configured Agent or an error string
///
/// # Notes
/// - Automatically fetches root key for local development (http:// URLs)
/// - Query signature verification is disabled: query responses are not
///   authenticated, so callers relying on integrity should use a cipher
///   (object content is covered by AES-GCM) or verify results themselves
pub async fn build_agent(host: &str, identity: Arc<dyn Identity>) -> Result<Agent, String> {
    let agent = Agent::builder()
        .with_url(host)
        .with_arc_identity(identity)
        .with_verify_query_signatures(false);

    let agent = if host.starts_with("https://") {
        agent
            .with_background_dynamic_routing()
            .build()
            .map_err(format_error)?
    } else {
        agent.build().map_err(format_error)?
    };

    if host.starts_with("http://") {
        // ignore errors for local development
        let _ = agent.fetch_root_key().await;
    }

    Ok(agent)
}
