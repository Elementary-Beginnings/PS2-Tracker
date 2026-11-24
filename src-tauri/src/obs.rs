//! Optional local endpoint (127.0.0.1 only) as an OBS browser source.
//! Serves the same overlay page as the overlay window plus snapshots (JSON/SSE).
//! Only requests addressed to this computer are answered: the `Host` header
//! must be `127.0.0.1:<port>`, `localhost:<port>` or `[::1]:<port>`
//! (what OBS sends for `http://127.0.0.1:<port>/`), anything else gets
//! 421 Misdirected Request. No CORS headers are set.

use std::convert::Infallible;
use std::net::SocketAddr;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::middleware::{Next, from_fn_with_state};
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
        let router = routes(app, obs.port);
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = rx.changed().await;
            })
            .await;
    });
}

/// Every route of the endpoint, behind the `Host` check.
fn routes(app: AppHandle, port: u16) -> Router {
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
    local_only(router, port)
}

/// Puts the `Host` check in front of every route, the fallback included.
fn local_only(router: Router, port: u16) -> Router {
    router.layer(from_fn_with_state(port, local_host_only))
}

/// `Host` (and an absolute request target, if any) names this computer and
/// the endpoint's port.
fn host_allowed(host: &str, port: u16) -> bool {
    let host = host.trim();
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|h| host.eq_ignore_ascii_case(&format!("{h}:{port}")))
}

/// Answers only requests addressed to 127.0.0.1/localhost on this port.
async fn local_host_only(State(port): State<u16>, req: Request, next: Next) -> Response {
    let header_ok = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|h| host_allowed(h, port));
    let target_ok = req
        .uri()
        .authority()
        .is_none_or(|a| host_allowed(a.as_str(), port));
    if header_ok && target_ok {
        next.run(req).await
    } else {
        StatusCode::MISDIRECTED_REQUEST.into_response()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn only_local_hosts_on_the_port() {
        assert!(host_allowed("127.0.0.1:47823", 47823));
        assert!(host_allowed("localhost:47823", 47823));
        assert!(host_allowed("LOCALHOST:47823", 47823));
        assert!(host_allowed("[::1]:47823", 47823));
        assert!(!host_allowed("127.0.0.1:47824", 47823));
        assert!(!host_allowed("127.0.0.1", 47823));
        assert!(!host_allowed("example.org:47823", 47823));
        assert!(!host_allowed("127.0.0.1.example.org:47823", 47823));
        assert!(!host_allowed("", 47823));
    }

    async fn status_of(port: u16, head: &str) -> String {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        s.write_all(head.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out.lines().next().unwrap_or("").to_owned()
    }

    #[tokio::test]
    async fn requests_for_another_host_are_refused() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = local_only(
            Router::new()
                .route("/api/snapshot", get(|| async { "{}" }))
                .fallback(|| async { StatusCode::NOT_FOUND }),
            port,
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        let req = |target: &str, host: Option<&str>| {
            let host = host.map(|h| format!("Host: {h}\r\n")).unwrap_or_default();
            format!("GET {target} HTTP/1.1\r\n{host}Connection: close\r\n\r\n")
        };
        // as OBS calls it
        for host in [
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ] {
            let line = status_of(port, &req("/api/snapshot", Some(&host))).await;
            assert!(line.contains(" 200 "), "{host}: {line}");
        }
        for host in [
            Some("example.org".to_owned()),
            Some(format!("example.org:{port}")),
            Some("127.0.0.1:1".to_owned()),
        ] {
            let line = status_of(port, &req("/api/snapshot", host.as_deref())).await;
            assert!(line.contains(" 421 "), "{host:?}: {line}");
        }
        // the fallback (overlay page, assets) is behind the same check
        let line = status_of(port, &req("/overlay.html", Some("example.org"))).await;
        assert!(line.contains(" 421 "), "{line}");
        // an absolute target must name this computer as well
        let line = status_of(
            port,
            &req(
                "http://example.org/api/snapshot",
                Some(&format!("127.0.0.1:{port}")),
            ),
        )
        .await;
        assert!(line.contains(" 421 "), "{line}");
    }
}
