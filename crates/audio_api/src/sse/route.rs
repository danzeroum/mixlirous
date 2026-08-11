use crate::middleware::AuthContext;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{self, Stream, StreamExt};
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

pub async fn job_stream(
    _auth: AuthContext,
    Path(job_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.hub.subscribe(job_id).await;

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let evt = Event::default()
                        .event(&event.event_type)
                        .json_data(event.data)
                        .unwrap_or_else(|_| Event::default().event("error"));
                    yield Ok(evt);
                }
                Err(_) => break,
            }
        }
    };

    let ready = Event::default()
        .event("stream.ready")
        .json_data(serde_json::json!({ "job_id": job_id, "resumed_from": null }))
        .unwrap_or_else(|_| Event::default().event("stream.ready"));

    let events = stream::once(async { Ok(ready) }).chain(stream);

    Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}