//! Optional local endpoint (127.0.0.1 only) as an OBS browser source.
//! Serves the same overlay page as the overlay window plus snapshots (JSON/SSE).

use std::convert::Infallible;
use std::net::SocketAddr;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use futures_util::{Stream, StreamExt};
use tauri::AppHandle;
use tokio::sync::watch;
use tokio_stream::wrappers::BroadcastStream;

use crate::engine::{current_snapshot, state};
use crate::state::lock;

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
                   img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; \
                   frame-ancestors 'none'";

pub fn restart(app: &AppHandle) {
    let st = state(app);
    if let Some(stop) = lock(&st.obs_stop).take() {
        let _ = stop.send(true);
    }
    let obs = st.cfg().obs.clone();
    if !obs.enabled {
        return;
    }
    let (tx, mut rx) = watch::channel(false);
    *lock(&st.obs_stop) = Some(tx);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], obs.port));
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(%addr, error = %e, "OBS endpoint not started");
                return;
            }
        };
        tracing::info!(%addr, "OBS endpoint running");
        let router = Router::new()
            .route(
                "/",
                get(|| async { Redirect::temporary("/overlay.html?obs=1") }),
            )
            .route("/api/snapshot", get(snapshot))
            .route("/api/events", get(events))
            .route("/api/background", get(background))
            .fallback(asset)
            .with_state(app);
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = rx.changed().await;
            })
            .await;
    });
}

async fn snapshot(State(app): State<AppHandle>) -> Response {
    let snap = current_snapshot(&state(&app));
    axum::Json(snap).into_response()
}

async fn background(State(app): State<AppHandle>) -> Response {
    let bg = lock(&state(&app).voice).background.clone();
    axum::Json(bg).into_response()
}

async fn events(
    State(app): State<AppHandle>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state(&app).obs_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|m| async move {
        m.ok()
            .map(|(kind, json)| Ok(Event::default().event(kind).data(json)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn asset(State(app): State<AppHandle>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    // only the overlay page and its assets – not the main UI
    if !(path == "overlay.html" || path.starts_with("assets/")) || path.contains("..") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match app.asset_resolver().get(path.to_owned()) {
        Some(a) => {
            let mut r = a.bytes().to_vec().into_response();
            let h = r.headers_mut();
            if let Ok(v) = HeaderValue::from_str(a.mime_type()) {
                h.insert(header::CONTENT_TYPE, v);
            }
            h.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static(CSP),
            );
            h.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            r
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
