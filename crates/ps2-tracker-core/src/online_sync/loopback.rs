//! Loopback redirect of the Discord sign-in (RFC 8252, section 7.3).
//!
//! Listens on `127.0.0.1` only, on the first free port of a fixed list (the
//! redirect URIs registered with Discord must match exactly), until one
//! request to `/callback` carries the expected `state` or the time runs out.
//! Then the listener is closed. Requests to other paths, with a wrong `state`
//! or a foreign `Host` get an error page and do not end the wait, so a local
//! web page cannot cancel a sign-in. The answer is static HTML without script.
//! Connections are served side by side (at most [`MAX_CONNECTIONS`] at a
//! time, each with a short read timeout), so an idle or slow connection does
//! not hold up the browser's callback.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Path of the redirect URI.
pub const CALLBACK_PATH: &str = "/callback";
/// Largest request head read.
const MAX_REQUEST: usize = 8 * 1024;
/// Time a single connection gets to send its request.
const READ_TIMEOUT: Duration = Duration::from_secs(2);
/// Connections served at the same time; further ones wait in the backlog.
pub const MAX_CONNECTIONS: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum CallbackError {
    #[error("none of the sign-in ports ({0}) is free on 127.0.0.1")]
    NoPort(String),
    #[error("the sign-in was not finished in time")]
    Timeout,
    #[error("Discord did not grant access ({0})")]
    Denied(String),
    #[error("network: {0}")]
    Io(#[from] std::io::Error),
}

/// What a callback request asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Callback {
    Code(String),
    /// `error` parameter of the authorization response (e.g. access_denied).
    Error(String),
}

pub struct Loopback {
    listener: TcpListener,
    port: u16,
}

impl Loopback {
    /// Binds the first free port of `ports` on 127.0.0.1.
    pub async fn bind(ports: &[u16]) -> Result<Self, CallbackError> {
        for &port in ports {
            if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
                let port = listener.local_addr()?.port();
                return Ok(Self { listener, port });
            }
        }
        let list: Vec<String> = ports.iter().map(u16::to_string).collect();
        Err(CallbackError::NoPort(list.join(", ")))
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `http://127.0.0.1:<port>/callback`
    pub fn redirect_uri(&self) -> String {
        redirect_uri(self.port)
    }

    /// Waits for the callback with `state` and returns the authorization
    /// code. The listener is closed when this returns.
    pub async fn wait(self, state: &str, timeout: Duration) -> Result<String, CallbackError> {
        let port = self.port;
        let state: std::sync::Arc<str> = state.into();
        // dropping the set (return, timeout) ends every open connection
        let mut conns = tokio::task::JoinSet::new();
        let work = async {
            loop {
                tokio::select! {
                    // when every slot is busy, new connections wait in the
                    // backlog until one frees up (read timeout at most)
                    accepted = self.listener.accept(), if conns.len() < MAX_CONNECTIONS => {
                        let (stream, _) = accepted?;
                        let state = state.clone();
                        conns.spawn(async move {
                            tokio::time::timeout(READ_TIMEOUT, handle(stream, port, &state)).await
                        });
                    }
                    Some(done) = conns.join_next(), if !conns.is_empty() => {
                        match done {
                            Ok(Ok(Ok(Some(Callback::Code(code))))) => return Ok(code),
                            Ok(Ok(Ok(Some(Callback::Error(e))))) => {
                                return Err(CallbackError::Denied(e));
                            }
                            // wrong path/state/host, broken request, slow client
                            _ => continue,
                        }
                    }
                }
            }
        };
        match tokio::time::timeout(timeout, work).await {
            Ok(r) => r,
            Err(_) => Err(CallbackError::Timeout),
        }
    }
}

pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{CALLBACK_PATH}")
}

async fn handle(
    mut stream: TcpStream,
    port: u16,
    state: &str,
) -> std::io::Result<Option<Callback>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_REQUEST {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf);
    let (status, page, result) = match parse_request(&head, port, state) {
        Ok(Callback::Code(c)) => ("200 OK", PAGE_DONE, Some(Callback::Code(c))),
        Ok(Callback::Error(e)) => ("200 OK", PAGE_DENIED, Some(Callback::Error(e))),
        Err(Reject::NotFound) => ("404 Not Found", PAGE_NOT_FOUND, None),
        Err(Reject::Bad) => ("400 Bad Request", PAGE_BAD, None),
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'\r\n\
         Connection: close\r\n\r\n{page}",
        page.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    let _ = stream.shutdown().await;
    Ok(result)
}

#[derive(Debug, PartialEq, Eq)]
enum Reject {
    NotFound,
    Bad,
}

/// Request head → callback, if it is the expected one.
fn parse_request(head: &str, port: u16, state: &str) -> Result<Callback, Reject> {
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    if method != "GET" || !target.starts_with('/') {
        return Err(Reject::Bad);
    }
    // the browser sends Host 127.0.0.1:<port>; anything else is not our page
    let host_ok = lines
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.trim().eq_ignore_ascii_case("host"))
        .all(|(_, v)| v.trim() == format!("127.0.0.1:{port}"));
    if !host_ok {
        return Err(Reject::Bad);
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| Reject::Bad)?;
    if url.path() != CALLBACK_PATH {
        return Err(Reject::NotFound);
    }
    let get = |k: &str| {
        url.query_pairs()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.into_owned())
    };
    if get("state").as_deref() != Some(state) {
        return Err(Reject::Bad);
    }
    if let Some(e) = get("error") {
        let e: String = e
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .take(64)
            .collect();
        return Ok(Callback::Error(e));
    }
    match get("code") {
        Some(c) if !c.is_empty() && c.len() <= 512 => Ok(Callback::Code(c)),
        _ => Err(Reject::Bad),
    }
}

macro_rules! page {
    ($title:literal, $text:literal) => {
        concat!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width\"><title>",
            $title,
            " – PS2 Session Tracker</title><style>body{font-family:sans-serif;",
            "max-width:32rem;margin:4rem auto;padding:0 1rem;line-height:1.5;",
            "color:#ddd;background:#16181d}</style></head><body><h1>",
            $title,
            "</h1><p>",
            $text,
            "</p></body></html>"
        )
    };
}

const PAGE_DONE: &str = page!(
    "Signed in",
    "You are signed in. You can close this tab and go back to the PS2 Session Tracker."
);
const PAGE_DENIED: &str = page!(
    "Sign-in cancelled",
    "The sign-in was cancelled. You can close this tab and try again in the PS2 Session Tracker."
);
const PAGE_NOT_FOUND: &str = page!("Not found", "Nothing here.");
const PAGE_BAD: &str = page!(
    "Not accepted",
    "This sign-in link is not valid. Start the sign-in again in the PS2 Session Tracker."
);

#[cfg(test)]
mod tests {
    use super::*;

    fn req(target: &str, host: &str) -> String {
        format!("GET {target} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: x\r\n\r\n")
    }

    #[test]
    fn parses_the_callback() {
        let h = "127.0.0.1:5000";
        assert_eq!(
            parse_request(&req("/callback?code=abc&state=s1", h), 5000, "s1"),
            Ok(Callback::Code("abc".into()))
        );
        assert_eq!(
            parse_request(
                &req("/callback?error=access_denied&state=s1", h),
                5000,
                "s1"
            ),
            Ok(Callback::Error("access_denied".into()))
        );
        assert_eq!(
            parse_request(&req("/callback?code=abc&state=other", h), 5000, "s1"),
            Err(Reject::Bad)
        );
        assert_eq!(
            parse_request(&req("/callback?code=abc", h), 5000, "s1"),
            Err(Reject::Bad)
        );
        assert_eq!(
            parse_request(&req("/favicon.ico", h), 5000, "s1"),
            Err(Reject::NotFound)
        );
        assert_eq!(
            parse_request(
                &req("/callback?code=abc&state=s1", "evil.example:5000"),
                5000,
                "s1"
            ),
            Err(Reject::Bad)
        );
        assert_eq!(
            parse_request(
                "POST /callback?code=a&state=s1 HTTP/1.1\r\n\r\n",
                5000,
                "s1"
            ),
            Err(Reject::Bad)
        );
        // an error value cannot inject markup into the UI message
        assert_eq!(
            parse_request(&req("/callback?error=%3Cb%3Ex&state=s1", h), 5000, "s1"),
            Ok(Callback::Error("bx".into()))
        );
    }

    #[test]
    fn pages_have_no_script() {
        for p in [PAGE_DONE, PAGE_DENIED, PAGE_NOT_FOUND, PAGE_BAD] {
            assert!(!p.to_ascii_lowercase().contains("<script"));
        }
    }

    async fn get(port: u16, target: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(req(target, &format!("127.0.0.1:{port}")).as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn waits_for_the_right_request_and_closes() {
        // port 0: any free port (the real list is fixed)
        let lb = Loopback::bind(&[0]).await.unwrap();
        let port = lb.port();
        assert_eq!(
            lb.redirect_uri(),
            format!("http://127.0.0.1:{port}/callback")
        );
        let task = tokio::spawn(lb.wait("st", Duration::from_secs(10)));
        assert!(get(port, "/").await.starts_with("HTTP/1.1 404"));
        assert!(
            get(port, "/callback?code=x&state=bad")
                .await
                .starts_with("HTTP/1.1 400")
        );
        let ok = get(port, "/callback?code=the-code&state=st").await;
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert!(ok.contains("Referrer-Policy: no-referrer"));
        assert_eq!(task.await.unwrap().unwrap(), "the-code");
        // the listener is gone
        assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
    }

    #[tokio::test]
    async fn times_out_and_reports_denial() {
        let lb = Loopback::bind(&[0]).await.unwrap();
        assert!(matches!(
            lb.wait("st", Duration::from_millis(50)).await,
            Err(CallbackError::Timeout)
        ));
        let lb = Loopback::bind(&[0]).await.unwrap();
        let port = lb.port();
        let task = tokio::spawn(lb.wait("st", Duration::from_secs(10)));
        get(port, "/callback?error=access_denied&state=st").await;
        assert!(
            matches!(task.await.unwrap(), Err(CallbackError::Denied(e)) if e == "access_denied")
        );
    }

    #[tokio::test]
    async fn idle_connections_do_not_hold_up_the_callback() {
        let lb = Loopback::bind(&[0]).await.unwrap();
        let port = lb.port();
        let task = tokio::spawn(lb.wait("st", Duration::from_secs(30)));
        // connections that send nothing, more than are served at once
        let mut idle = Vec::new();
        for _ in 0..MAX_CONNECTIONS + 4 {
            idle.push(TcpStream::connect(("127.0.0.1", port)).await.unwrap());
        }
        let started = tokio::time::Instant::now();
        // the slots free up after the short read timeout at the latest
        let ok = tokio::time::timeout(
            READ_TIMEOUT * 3,
            get(port, "/callback?code=the-code&state=st"),
        )
        .await
        .expect("callback answered");
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert_eq!(task.await.unwrap().unwrap(), "the-code");
        assert!(started.elapsed() < READ_TIMEOUT * 3);
        drop(idle);
    }

    #[tokio::test]
    async fn a_slow_connection_does_not_delay_the_callback() {
        let lb = Loopback::bind(&[0]).await.unwrap();
        let port = lb.port();
        let task = tokio::spawn(lb.wait("st", Duration::from_secs(30)));
        let mut slow = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        slow.write_all(b"GET /callback?code=x").await.unwrap();
        // well within the read timeout of the slow one
        let ok = tokio::time::timeout(
            READ_TIMEOUT / 2,
            get(port, "/callback?code=the-code&state=st"),
        )
        .await
        .expect("callback answered while another connection is open");
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert_eq!(task.await.unwrap().unwrap(), "the-code");
    }

    #[tokio::test]
    async fn takes_the_next_free_port() {
        let busy = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let taken = busy.local_addr().unwrap().port();
        let lb = Loopback::bind(&[taken, 0]).await.unwrap();
        assert_ne!(lb.port(), taken);
        assert!(matches!(
            Loopback::bind(&[taken]).await,
            Err(CallbackError::NoPort(_))
        ));
    }
}
