//! Loopback-only API protection. Reads are same-origin-protected by the browser; mutations
//! additionally require the per-launch session token in a custom header, which no cross-site
//! page can add without a CORS preflight (which is never answered).

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;

use super::AppState;

pub const SESSION_HEADER: &str = "x-switcheroo-session";

pub fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn same_origin(headers: &HeaderMap, port: u16, dev: bool) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else { return true };
    if dev {
        return true;
    }
    let allowed = [format!("http://127.0.0.1:{port}"), format!("http://localhost:{port}")];
    allowed.iter().any(|a| a.eq_ignore_ascii_case(origin))
}

pub async fn guard(State(st): State<AppState>, req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path();
    if !path.starts_with("/api/") {
        return next.run(req).await;
    }
    let mutating = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if mutating {
        if !same_origin(req.headers(), st.port, st.dev) {
            return (StatusCode::FORBIDDEN, "cross-origin request").into_response();
        }
        let ok = req
            .headers()
            .get(SESSION_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|v| bool::from(v.as_bytes().ct_eq(st.session.as_bytes())))
            .unwrap_or(false);
        if !ok {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "error": "missing or invalid X-Switcheroo-Session header" })),
            )
                .into_response();
        }
    }
    next.run(req).await
}
