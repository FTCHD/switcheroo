//! The embedded web bundle with an SPA fallback. `index.html` gets `window.__SWITCHEROO__`
//! injected at the `<!-- switcheroo:boot -->` marker so the page knows the session token and
//! platform facts before its first render.

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

use super::AppState;

#[derive(rust_embed::Embed)]
#[folder = "web/dist"]
struct Assets;

const MARKER: &str = "<!-- switcheroo:boot -->";

pub fn boot_script(st: &AppState) -> String {
    let payload = serde_json::json!({
        "session": st.session.as_str(),
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "dev": st.dev,
        "vault": st.core.vault.name(),
        "dataDir": st.core.dirs.data.display().to_string(),
    });
    format!("<script>window.__SWITCHEROO__ = {payload};</script>")
}

fn inject(html: &str, script: &str) -> String {
    if html.contains(MARKER) {
        html.replacen(MARKER, script, 1)
    } else if let Some(i) = html.find("</head>") {
        format!("{}{}{}", &html[..i], script, &html[i..])
    } else {
        format!("{script}{html}")
    }
}

pub async fn serve(State(st): State<AppState>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let (path, file) = match Assets::get(path) {
        Some(f) => (path, f),
        None => match Assets::get("index.html") {
            Some(f) => ("index.html", f),
            None => return (StatusCode::NOT_FOUND, "web bundle missing; build web/ first").into_response(),
        },
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut resp = if path == "index.html" {
        Body::from(inject(&String::from_utf8_lossy(&file.data), &boot_script(&st))).into_response()
    } else {
        Body::from(file.data.into_owned()).into_response()
    };
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref()).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    resp.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    resp
}
