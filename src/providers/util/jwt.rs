//! Read claims from a JWT without verifying it (we only need the email/plan the issuer put in
//! the token the CLI already trusts).

use anyhow::{Context, Result, anyhow};
use base64::Engine;

pub fn claims(token: &str) -> Result<serde_json::Value> {
    let payload = token.split('.').nth(1).ok_or_else(|| anyhow!("not a JWT"))?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .context("decoding JWT payload")?;
    serde_json::from_slice(&bytes).context("parsing JWT payload")
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn reads_claims() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"email":"a@b.c","https://api.openai.com/auth":{"chatgpt_plan_type":"plus"}}"#);
        let token = format!("eyJhbGciOiJIUzI1NiJ9.{payload}.sig");
        let c = claims(&token).unwrap();
        assert_eq!(c["email"], "a@b.c");
        assert_eq!(c["https://api.openai.com/auth"]["chatgpt_plan_type"], "plus");
    }
}
