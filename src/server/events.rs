//! `GET /api/events`: server-sent events from the core bus.

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use super::AppState;

pub async fn sse(State(st): State<AppState>) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let rx = st.core.bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|item| async move {
        match item {
            Ok(ev) => {
                Some(Ok(Event::default().event(ev.kind.clone()).json_data(&ev).unwrap_or_else(|_| Event::default())))
            }
            Err(_) => None, // lagged; the client refetches on the next event anyway
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
