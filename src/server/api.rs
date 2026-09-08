//! JSON handlers. Core calls block (they may run CLIs), so each runs on the blocking pool.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use super::AppState;
use crate::core::Core;
use crate::core::settings::Settings;
use crate::core::switch::OpError;

pub struct ApiError(anyhow::Error);

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0.downcast_ref::<OpError>() {
            Some(OpError::NotInstalled(..)) => StatusCode::NOT_FOUND,
            Some(OpError::NothingLoggedIn(..)) => StatusCode::CONFLICT,
            None => {
                let msg = format!("{}", self.0);
                if msg.starts_with("unknown provider") || msg.contains("no account matching") {
                    StatusCode::NOT_FOUND
                } else if msg.contains("ambiguous") || msg.contains("no saved accounts") {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
        };
        (status, Json(serde_json::json!({ "error": format!("{:#}", self.0) }))).into_response()
    }
}

type ApiResult = Result<Response, ApiError>;

async fn blocking<T: serde::Serialize + Send + 'static>(
    core: Arc<Core>,
    f: impl FnOnce(&Core) -> anyhow::Result<T> + Send + 'static,
) -> ApiResult {
    let value = tokio::task::spawn_blocking(move || f(&core))
        .await
        .map_err(|e| ApiError(anyhow::anyhow!("task failed: {e}")))??;
    Ok(Json(value).into_response())
}

pub async fn status(State(st): State<AppState>) -> ApiResult {
    let vault = st.core.vault.name();
    Ok(Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "vault": vault,
        "dataDir": st.core.dirs.data.display().to_string(),
        "revision": st.core.revision(),
        "dev": st.dev,
    }))
    .into_response())
}

#[derive(Deserialize, Default)]
pub struct RefreshQuery {
    #[serde(default)]
    refresh: bool,
}

pub async fn providers(State(st): State<AppState>, Query(q): Query<RefreshQuery>) -> ApiResult {
    blocking(st.core, move |c| Ok(c.status_all(q.refresh))).await
}

pub async fn provider(State(st): State<AppState>, Path(id): Path<String>, Query(q): Query<RefreshQuery>) -> ApiResult {
    blocking(st.core, move |c| Ok(c.status(c.provider(&id)?, q.refresh))).await
}

#[derive(Deserialize, Default)]
pub struct SaveBody {
    label: Option<String>,
}

pub async fn save(State(st): State<AppState>, Path(id): Path<String>, body: Option<Json<SaveBody>>) -> ApiResult {
    let label = body.and_then(|b| b.0.label).filter(|l| !l.trim().is_empty());
    blocking(st.core, move |c| c.save(&id, label)).await
}

#[derive(Deserialize)]
pub struct UseBody {
    account: String,
}

pub async fn use_account(State(st): State<AppState>, Path(id): Path<String>, Json(body): Json<UseBody>) -> ApiResult {
    blocking(st.core, move |c| c.use_account(&id, &body.account)).await
}

pub async fn login(State(st): State<AppState>, Path(id): Path<String>) -> ApiResult {
    blocking(st.core, move |c| {
        let (spawned, command) = c.login_spawn(&id)?;
        Ok(serde_json::json!({ "spawned": spawned, "command": command }))
    })
    .await
}

pub async fn refresh(State(st): State<AppState>, Path(id): Path<String>) -> ApiResult {
    blocking(st.core, move |c| Ok(c.status(c.provider(&id)?, true))).await
}

#[derive(Deserialize)]
pub struct RenameBody {
    label: String,
}

pub async fn rename(
    State(st): State<AppState>,
    Path((provider, account)): Path<(String, String)>,
    Json(body): Json<RenameBody>,
) -> ApiResult {
    blocking(st.core, move |c| c.rename(&provider, &account, &body.label)).await
}

pub async fn remove(State(st): State<AppState>, Path((provider, account)): Path<(String, String)>) -> ApiResult {
    blocking(st.core, move |c| c.remove(&provider, &account)).await
}

pub async fn get_settings(State(st): State<AppState>) -> ApiResult {
    let s = st.core.settings.read().map(|s| s.clone()).unwrap_or_default();
    Ok(Json(s).into_response())
}

pub async fn put_settings(State(st): State<AppState>, Json(settings): Json<Settings>) -> ApiResult {
    blocking(st.core, move |c| {
        c.save_settings(settings.clone())?;
        Ok(settings)
    })
    .await
}

pub async fn doctor(State(st): State<AppState>) -> ApiResult {
    blocking(st.core, move |c| Ok(c.doctor())).await
}
