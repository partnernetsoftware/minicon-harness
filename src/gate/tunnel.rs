//! The tunnel plane: how a request actually leaves the gate.
//!
//! `direct` connects straight to the endpoint. `env` and an explicit `{url,
//! noProxy}` go through a proxy: the gate opens the TCP connection to the
//! proxy, writes `CONNECT host:port`, checks for a 2xx, and only then hands the
//! established stream to TLS. **The tunnel is the unit of the per-lane decision**
//! -- a proxy chosen by an HTTP client's own global setting could never be
//! scoped to one credential.
//!
//! G1 (recorded in the plan's W3 record): `ureq` 2 exposes
//! `AgentBuilder::tls_connector` over the public `TlsConnector` seam, so a
//! stream the gate established reaches a custom connector. The connector itself
//! must speak `rustls` types, which is the one dependency question the plan
//! records; this module is written so that the *only* place that needs it is
//! `tls_connector()`.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::config::Proxy;

/// How long a proxy may take to answer `CONNECT`.
pub const TUNNEL_TIMEOUT: Duration = Duration::from_secs(15);
/// Largest proxy head the gate will read before giving up.
pub const MAX_PROXY_HEAD: usize = 8 * 1024;

/// The resolved decision for one attempt: where the bytes go first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Route {
    /// Connect straight to the endpoint host.
    Direct,
    /// Go through this proxy, after checking the bypass list.
    Proxied {
        /// `host:port` of the proxy.
        authority: String,
        /// `user:pass`, when the proxy URL carried credentials.
        credentials: Option<(String, String)>,
    },
}

impl Route {
    /// The `x-gate-egress` value: a host and port, never a URL.
    pub fn host(&self) -> String {
        match self {
            Route::Direct => String::new(),
            Route::Proxied { authority, .. } => authority.clone(),
        }
    }
}

/// Splits a proxy URL into its authority and optional credentials, dropping the
/// scheme and any path. **The credentials never leave this function or the
/// `CONNECT` header.**
pub fn parse_proxy_url(url: &str) -> (String, Option<(String, String)>) {
    let after_scheme = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url);
    let authority_part = after_scheme.split('/').next().unwrap_or("");
    match authority_part.rsplit_once('@') {
        Some((userinfo, host)) => {
            let (user, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
            (
                host.to_owned(),
                Some((user.to_owned(), password.to_owned())),
            )
        }
        None => (authority_part.to_owned(), None),
    }
}

/// Whether `host` is on the egress's bypass list.
///
/// A bare entry matches the host exactly; an entry with a leading dot matches
/// that suffix (`.internal.example` matches `a.internal.example`).
pub fn bypasses(host: &str, no_proxy: &[String]) -> bool {
    // A bracketed IPv6 authority (`[::1]:8787`) keeps its brackets until the
    // port is stripped, otherwise the `]` lands inside the host.
    let bare = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(rest),
        None => match host.rsplit_once(':') {
            Some((head, tail)) if tail.chars().all(|c| c.is_ascii_digit()) && !head.contains(':') => head,
            _ => host,
        },
    };
    no_proxy.iter().any(|entry| {
        let entry = entry.trim();
        if entry.is_empty() {
            return false;
        }
        let entry = entry.trim_start_matches('*');
        if let Some(suffix) = entry.strip_prefix('.') {
            bare == suffix || bare.ends_with(&format!(".{suffix}"))
        } else {
            entry.eq_ignore_ascii_case(bare)
        }
    })
}

/// Resolves one egress into a route for `endpoint_host`.
///
/// `env_proxy` is the process's proxy variable value, injected so the decision
/// is testable and the gate's own environment never leaks into a request it did
/// not ask for.
pub fn route_for(
    proxy: &Proxy,
    endpoint_host: &str,
    env_proxy: Option<&str>,
    env_no_proxy: &[String],
) -> Result<Route, String> {
    match proxy {
        Proxy::Direct => Ok(Route::Direct),
        Proxy::Env => match env_proxy {
            // `env` is the escape hatch: it inherits the process variables, and
            // the same bypass list still applies.
            None => Ok(Route::Direct),
            Some(url) => {
                if bypasses(endpoint_host, env_no_proxy) {
                    return Ok(Route::Direct);
                }
                let (authority, credentials) = parse_proxy_url(url);
                if authority.is_empty() {
                    return Ok(Route::Direct);
                }
                Ok(Route::Proxied { authority, credentials })
            }
        },
        Proxy::Url { url, no_proxy } => {
            if bypasses(endpoint_host, no_proxy) {
                return Ok(Route::Direct);
            }
            let (authority, credentials) = parse_proxy_url(url);
            if authority.is_empty() {
                return Err(String::from("the egress proxy URL has no authority"));
            }
            Ok(Route::Proxied { authority, credentials })
        }
    }
}

/// The `CONNECT` request one tunnel sends.
pub fn connect_request(authority: &str, credentials: Option<&(String, String)>) -> String {
    let mut request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: keep-alive\r\n"
    );
    if let Some((user, password)) = credentials {
        request.push_str(&format!(
            "Proxy-Authorization: Basic {}\r\n",
            base64(&format!("{user}:{password}"))
        ));
    }
    request.push_str("\r\n");
    request
}

/// Minimal base64, so a proxy credential costs no dependency.
pub fn base64(input: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) as usize & 63] as char);
        out.push(TABLE[(triple >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[triple as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The status a proxy answered a `CONNECT` with, or a transport failure.
pub fn open_tunnel(
    route: &Route,
    endpoint_authority: &str,
) -> Result<TcpStream, TunnelError> {
    match route {
        Route::Direct => {
            let mut last = None;
            for address in endpoint_authority.to_socket_addrs().map_err(|error| {
                TunnelError {
                    status: None,
                    reason: format!("cannot resolve `{endpoint_authority}`: {error}"),
                }
            })? {
                match TcpStream::connect_timeout(&address, TUNNEL_TIMEOUT) {
                    Ok(stream) => return tune(stream),
                    Err(error) => last = Some(error),
                }
            }
            Err(TunnelError {
                status: None,
                reason: format!(
                    "cannot connect to `{endpoint_authority}`: {}",
                    last.map(|error| error.to_string()).unwrap_or_default()
                ),
            })
        }
        Route::Proxied {
            authority,
            credentials,
        } => {
            let stream = connect_any(authority)?;
            let stream = tune(stream)?;
            let mut tunnel = stream;
            tunnel
                .write_all(connect_request(endpoint_authority, credentials.as_ref()).as_bytes())
                .map_err(|error| TunnelError {
                    status: None,
                    reason: format!("cannot write CONNECT: {error}"),
                })?;
            tunnel.flush().map_err(|error| TunnelError {
                status: None,
                reason: format!("cannot flush CONNECT: {error}"),
            })?;
            let status = read_connect_reply(&mut tunnel)?;
            if (200..300).contains(&status) {
                tune(tunnel)
            } else {
                Err(TunnelError {
                    status: Some(status),
                    reason: if status == 407 {
                        String::from("the proxy answered 407 (proxy authentication required)")
                    } else {
                        format!("the proxy refused CONNECT with {status}")
                    },
                })
            }
        }
    }
}

fn connect_any(authority: &str) -> Result<TcpStream, TunnelError> {
    let mut last = None;
    let addresses = authority.to_socket_addrs().map_err(|error| TunnelError {
        status: None,
        reason: format!("cannot resolve the proxy `{authority}`: {error}"),
    })?;
    for address in addresses {
        match TcpStream::connect_timeout(&address, TUNNEL_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = Some(error),
        }
    }
    Err(TunnelError {
        status: None,
        reason: format!(
            "cannot connect to the proxy `{authority}`: {}",
            last.map(|error| error.to_string()).unwrap_or_default()
        ),
    })
}

fn tune(stream: TcpStream) -> Result<TcpStream, TunnelError> {
    stream.set_nodelay(true).ok();
    Ok(stream)
}

/// Reads one `CONNECT` reply head and returns its status. The body is empty by
/// definition, so the tunnel starts right after the blank line.
fn read_connect_reply(stream: &mut TcpStream) -> Result<u16, TunnelError> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head.len() < MAX_PROXY_HEAD {
        let read = stream.read(&mut byte).map_err(|error| TunnelError {
            status: None,
            reason: format!("cannot read the CONNECT reply: {error}"),
        })?;
        if read == 0 {
            break;
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&head).to_string();
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| TunnelError {
            status: None,
            reason: String::from("the proxy reply has no status line"),
        })?;
    Ok(status)
}

/// A tunnel failure: an optional proxy status and a reason with no credential
/// bytes in it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TunnelError {
    /// The proxy's status, when it answered at all.
    pub status: Option<u16>,
    /// One sentence; never contains a proxy credential.
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_urls_split_into_authority_and_credentials() {
        assert_eq!(
            parse_proxy_url("http://127.0.0.1:8891"),
            (String::from("127.0.0.1:8891"), None)
        );
        assert_eq!(
            parse_proxy_url("http://user:pass@10.0.0.2:3128/some/path"),
            (
                String::from("10.0.0.2:3128"),
                Some((String::from("user"), String::from("pass")))
            )
        );
    }

    #[test]
    fn the_connect_request_carries_basic_auth_only_when_needed() {
        let plain = connect_request("api.example.test:443", None);
        assert!(plain.starts_with("CONNECT api.example.test:443 HTTP/1.1\r\n"));
        assert!(!plain.contains("Proxy-Authorization"));
        let authenticated =
            connect_request("api.example.test:443", Some(&(String::from("u"), String::from("p"))));
        assert!(authenticated.contains("Proxy-Authorization: Basic dTpw\r\n"));
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(""), "");
        assert_eq!(base64("f"), "Zg==");
        assert_eq!(base64("fo"), "Zm8=");
        assert_eq!(base64("foo"), "Zm9v");
        assert_eq!(base64("user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn bypass_matches_exact_and_suffix_entries() {
        let list = vec![
            String::from("localhost"),
            String::from("127.0.0.1"),
            String::from("::1"),
            String::from(".internal.example"),
        ];
        assert!(bypasses("localhost", &list));
        assert!(bypasses("localhost:8787", &list));
        assert!(bypasses("127.0.0.1:9", &list));
        assert!(bypasses("[::1]:8787", &list));
        assert!(bypasses("a.internal.example", &list));
        assert!(bypasses("internal.example", &list));
        assert!(!bypasses("api.openai.com", &list));
        assert!(!bypasses("example.com", &list));
    }

    #[test]
    fn direct_never_yields_a_proxy_even_with_env_set() {
        let route = route_for(&Proxy::Direct, "api.example.test", Some("http://1.2.3.4:1"), &[]).expect("route");
        assert_eq!(route, Route::Direct);
    }

    #[test]
    fn env_inherits_the_process_proxy_but_respects_bypass() {
        let route = route_for(&Proxy::Env, "api.example.test", Some("http://1.2.3.4:1"), &[]).expect("route");
        assert_eq!(
            route,
            Route::Proxied {
                authority: String::from("1.2.3.4:1"),
                credentials: None
            }
        );
        let bypassed = route_for(
            &Proxy::Env,
            "api.example.test",
            Some("http://1.2.3.4:1"),
            &[String::from("api.example.test")],
        )
        .expect("route");
        assert_eq!(bypassed, Route::Direct);
    }

    #[test]
    fn env_without_a_process_proxy_is_direct() {
        assert_eq!(
            route_for(&Proxy::Env, "api.example.test", None, &[]).expect("route"),
            Route::Direct
        );
    }

    #[test]
    fn an_explicit_egress_keeps_its_bypass_list() {
        let proxy = Proxy::Url {
            url: String::from("http://127.0.0.1:8891"),
            no_proxy: vec![String::from("127.0.0.1")],
        };
        assert_eq!(
            route_for(&proxy, "127.0.0.1", None, &[]).expect("route"),
            Route::Direct,
            "loopback is never proxied"
        );
        assert_eq!(
            route_for(&proxy, "api.openai.com", None, &[]).expect("route"),
            Route::Proxied {
                authority: String::from("127.0.0.1:8891"),
                credentials: None
            }
        );
    }

    #[test]
    fn the_route_host_is_a_host_and_port_not_a_url() {
        let route = route_for(
            &Proxy::Url {
                url: String::from("http://user:secret@10.0.0.2:3128"),
                no_proxy: vec![],
            },
            "api.example.test",
            None,
            &[],
        )
        .expect("route");
        let host = route.host();
        assert_eq!(host, "10.0.0.2:3128");
        assert!(!host.contains("secret"));
    }
}
