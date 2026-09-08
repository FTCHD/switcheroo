//! Tiny JSON GET for provider usage endpoints. Providers hand over a bearer token they read
//! from their own slot; nothing here logs or returns it.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde_json::Value;

pub struct Response {
    pub status: u16,
    pub json: Option<Value>,
}

pub fn get_json(url: &str, headers: &[(&str, &str)]) -> Result<Response> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = agent.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.call().with_context(|| format!("GET {url}"))?;
    let status = resp.status().as_u16();
    let json = resp.body_mut().read_json::<Value>().ok();
    Ok(Response { status, json })
}

/// Turn a non-2xx into an error with a short, token-free message.
pub fn ensure_ok<'a>(resp: &'a Response, what: &str) -> Result<&'a Value> {
    if !(200..300).contains(&resp.status) {
        return Err(anyhow!("{what}: HTTP {}", resp.status));
    }
    resp.json.as_ref().ok_or_else(|| anyhow!("{what}: response was not JSON"))
}
